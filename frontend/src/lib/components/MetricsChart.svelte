<script lang="ts">
  import { onMount } from 'svelte';
  import * as d3 from 'd3';
  import type { DailyMetric } from '../api';

  interface Props {
    daily: DailyMetric[];
    days: number;
  }

  let { daily, days }: Props = $props();

  let svgEl: SVGSVGElement;
  let containerEl: HTMLDivElement;

  function render() {
    if (!svgEl || !containerEl) return;

    const width = containerEl.clientWidth;
    const height = 220;
    const margin = { top: 16, right: 16, bottom: 32, left: 40 };
    const innerW = width - margin.left - margin.right;
    const innerH = height - margin.top - margin.bottom;

    const svg = d3.select(svgEl);
    svg.selectAll('*').remove();
    svg.attr('width', width).attr('height', height);

    // Group data by event_type
    const claimedData = daily
      .filter(d => d.event_type === 'issue_claimed')
      .map(d => ({ day: new Date(d.day), count: d.count }));
    const completedData = daily
      .filter(d => d.event_type === 'agent_completed')
      .map(d => ({ day: new Date(d.day), count: d.count }));

    const allDays = [...claimedData, ...completedData].map(d => d.day);
    const allCounts = [...claimedData, ...completedData].map(d => d.count);

    if (allDays.length === 0) {
      svg.append('text')
        .attr('x', width / 2)
        .attr('y', height / 2)
        .attr('text-anchor', 'middle')
        .attr('fill', '#6b7280')
        .attr('font-size', '14px')
        .text('No metrics data yet');
      return;
    }

    const xExtent = d3.extent(allDays) as [Date, Date];
    const x = d3.scaleTime().domain(xExtent).range([0, innerW]);
    const y = d3.scaleLinear()
      .domain([0, d3.max(allCounts) ?? 1])
      .nice()
      .range([innerH, 0]);

    const g = svg.append('g').attr('transform', `translate(${margin.left},${margin.top})`);

    // X axis
    g.append('g')
      .attr('transform', `translate(0,${innerH})`)
      .call(d3.axisBottom(x).ticks(Math.min(days, 7)).tickFormat(d3.timeFormat('%b %d') as any))
      .selectAll('text,line,path')
      .attr('stroke', '#4b5563')
      .attr('fill', '#9ca3af')
      .attr('font-size', '11px');

    // Y axis
    g.append('g')
      .call(d3.axisLeft(y).ticks(5).tickFormat(d3.format('d')))
      .selectAll('text,line,path')
      .attr('stroke', '#4b5563')
      .attr('fill', '#9ca3af')
      .attr('font-size', '11px');

    const line = d3.line<{ day: Date; count: number }>()
      .x(d => x(d.day))
      .y(d => y(d.count))
      .curve(d3.curveMonotoneX);

    // Issue claimed line (blue)
    if (claimedData.length > 0) {
      g.append('path')
        .datum(claimedData.sort((a, b) => a.day.getTime() - b.day.getTime()))
        .attr('fill', 'none')
        .attr('stroke', '#3b82f6')
        .attr('stroke-width', 2)
        .attr('d', line);
    }

    // Agent completed line (green)
    if (completedData.length > 0) {
      g.append('path')
        .datum(completedData.sort((a, b) => a.day.getTime() - b.day.getTime()))
        .attr('fill', 'none')
        .attr('stroke', '#22c55e')
        .attr('stroke-width', 2)
        .attr('d', line);
    }

    // Legend
    const legend = svg.append('g').attr('transform', `translate(${margin.left + 8}, ${margin.top})`);
    legend.append('line').attr('x1', 0).attr('x2', 16).attr('y1', 0).attr('y2', 0).attr('stroke', '#3b82f6').attr('stroke-width', 2);
    legend.append('text').attr('x', 20).attr('y', 4).attr('fill', '#9ca3af').attr('font-size', '11px').text('Issues claimed');
    legend.append('line').attr('x1', 120).attr('x2', 136).attr('y1', 0).attr('y2', 0).attr('stroke', '#22c55e').attr('stroke-width', 2);
    legend.append('text').attr('x', 140).attr('y', 4).attr('fill', '#9ca3af').attr('font-size', '11px').text('Agents completed');
  }

  onMount(() => {
    render();
    const observer = new ResizeObserver(() => render());
    observer.observe(containerEl);
    return () => observer.disconnect();
  });

  $effect(() => {
    // Re-render when data changes
    daily;
    days;
    render();
  });
</script>

<div bind:this={containerEl} class="w-full">
  <svg bind:this={svgEl}></svg>
</div>
