<script lang="ts">
  import { onMount } from 'svelte';
  import * as d3 from 'd3';
  import type { DailyMetric } from '../api';

  interface Props {
    daily: DailyMetric[];
  }

  let { daily }: Props = $props();

  let svgEl: SVGSVGElement;
  let containerEl: HTMLDivElement;

  const eventColors: Record<string, string> = {
    agent_spawned: '#8b5cf6',
    issue_claimed: '#3b82f6',
    agent_completed: '#22c55e',
    stage_started: '#f59e0b',
    merge_attempted: '#06b6d4',
  };

  const eventLabels: Record<string, string> = {
    agent_spawned: 'Spawned',
    issue_claimed: 'Claimed',
    agent_completed: 'Completed',
    stage_started: 'Stages',
    merge_attempted: 'Merges',
  };

  function render() {
    if (!svgEl || !containerEl) return;

    const width = containerEl.clientWidth;
    const height = 200;
    const margin = { top: 12, right: 12, bottom: 28, left: 32 };
    const innerW = width - margin.left - margin.right;
    const innerH = height - margin.top - margin.bottom;

    const svg = d3.select(svgEl);
    svg.selectAll('*').remove();
    svg.attr('width', width).attr('height', height);

    if (daily.length === 0) {
      svg.append('text')
        .attr('x', width / 2).attr('y', height / 2)
        .attr('text-anchor', 'middle')
        .attr('fill', '#6b7280').attr('font-size', '12px')
        .text('No activity data');
      return;
    }

    // Group by day, stack by event_type
    const days = [...new Set(daily.map(d => d.day))].sort();
    const types = [...new Set(daily.map(d => d.event_type))].filter(t => t in eventColors);

    const dayData = days.map(day => {
      const row: Record<string, number> = { _day: 0 };
      for (const t of types) {
        const match = daily.find(d => d.day === day && d.event_type === t);
        row[t] = match?.count ?? 0;
      }
      return { day, ...row };
    });

    const stack = d3.stack<any>().keys(types)(dayData);

    const x = d3.scaleBand()
      .domain(days)
      .range([0, innerW])
      .padding(0.3);

    const yMax = d3.max(stack, s => d3.max(s, d => d[1])) ?? 1;
    const y = d3.scaleLinear().domain([0, yMax]).nice().range([innerH, 0]);

    const g = svg.append('g').attr('transform', `translate(${margin.left},${margin.top})`);

    // Axes
    g.append('g')
      .attr('transform', `translate(0,${innerH})`)
      .call(d3.axisBottom(x).tickFormat(d => {
        const date = new Date(d);
        return `${date.getMonth() + 1}/${date.getDate()}`;
      }))
      .selectAll('text,line,path')
      .attr('stroke', '#4b5563').attr('fill', '#9ca3af').attr('font-size', '10px');

    g.append('g')
      .call(d3.axisLeft(y).ticks(4).tickFormat(d3.format('d')))
      .selectAll('text,line,path')
      .attr('stroke', '#4b5563').attr('fill', '#9ca3af').attr('font-size', '10px');

    // Bars
    g.selectAll('g.layer')
      .data(stack)
      .join('g')
      .attr('class', 'layer')
      .attr('fill', d => eventColors[d.key] ?? '#6b7280')
      .selectAll('rect')
      .data(d => d)
      .join('rect')
      .attr('x', d => x(d.data.day) ?? 0)
      .attr('y', d => y(d[1]))
      .attr('height', d => y(d[0]) - y(d[1]))
      .attr('width', x.bandwidth())
      .attr('rx', 2);

    // Legend
    const legend = svg.append('g').attr('transform', `translate(${margin.left}, ${height - 6})`);
    types.forEach((t, i) => {
      const xOff = i * 80;
      legend.append('rect').attr('x', xOff).attr('y', -8).attr('width', 8).attr('height', 8).attr('rx', 2).attr('fill', eventColors[t]);
      legend.append('text').attr('x', xOff + 12).attr('y', 0).attr('fill', '#9ca3af').attr('font-size', '9px').text(eventLabels[t] ?? t);
    });
  }

  onMount(() => {
    render();
    const observer = new ResizeObserver(() => render());
    observer.observe(containerEl);
    return () => observer.disconnect();
  });

  $effect(() => { daily; render(); });
</script>

<div class="rounded-xl bg-gray-900 border border-gray-800 p-4">
  <h4 class="text-xs font-semibold text-gray-400 uppercase tracking-wider mb-2">Agent Activity</h4>
  <div bind:this={containerEl} class="w-full">
    <svg bind:this={svgEl}></svg>
  </div>
</div>
