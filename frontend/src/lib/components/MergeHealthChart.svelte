<script lang="ts">
  import { onMount } from 'svelte';
  import * as d3 from 'd3';

  interface Props {
    total: number;
    clean: number;
    conflicted: number;
    escalated: number;
  }

  let { total, clean, conflicted, escalated }: Props = $props();

  let svgEl: SVGSVGElement;
  let containerEl: HTMLDivElement;

  const sliceColors = ['#22c55e', '#f59e0b', '#ef4444'];
  const sliceLabels = ['Clean', 'Conflicted', 'Escalated'];

  function render() {
    if (!svgEl || !containerEl) return;

    const size = Math.min(containerEl.clientWidth, 200);
    const radius = size / 2 - 8;
    const innerRadius = radius * 0.55;

    const svg = d3.select(svgEl);
    svg.selectAll('*').remove();
    svg.attr('width', size).attr('height', size);

    if (total === 0) {
      svg.append('text')
        .attr('x', size / 2).attr('y', size / 2)
        .attr('text-anchor', 'middle').attr('dominant-baseline', 'middle')
        .attr('fill', '#6b7280').attr('font-size', '12px')
        .text('No merges');
      return;
    }

    const data = [clean, conflicted, escalated];
    const pie = d3.pie<number>().sort(null).padAngle(0.02);
    const arc = d3.arc<d3.PieArcDatum<number>>()
      .innerRadius(innerRadius)
      .outerRadius(radius)
      .cornerRadius(3);

    const g = svg.append('g')
      .attr('transform', `translate(${size / 2},${size / 2})`);

    g.selectAll('path')
      .data(pie(data))
      .join('path')
      .attr('d', arc as any)
      .attr('fill', (_, i) => sliceColors[i])
      .attr('opacity', 0.85);

    // Center text
    g.append('text')
      .attr('text-anchor', 'middle')
      .attr('dominant-baseline', 'middle')
      .attr('fill', '#e5e7eb')
      .attr('font-size', '20px')
      .attr('font-weight', '600')
      .text(total.toString());

    g.append('text')
      .attr('text-anchor', 'middle')
      .attr('y', 18)
      .attr('fill', '#9ca3af')
      .attr('font-size', '10px')
      .text('merges');

    // Legend below
    const legend = svg.append('g')
      .attr('transform', `translate(${size / 2 - 60}, ${size - 4})`);

    data.forEach((val, i) => {
      if (val === 0) return;
      const xOff = i * 72;
      legend.append('circle').attr('cx', xOff + 4).attr('cy', -4).attr('r', 4).attr('fill', sliceColors[i]);
      legend.append('text').attr('x', xOff + 12).attr('y', 0).attr('fill', '#9ca3af').attr('font-size', '9px')
        .text(`${sliceLabels[i]} ${val}`);
    });
  }

  onMount(() => {
    render();
    const observer = new ResizeObserver(() => render());
    observer.observe(containerEl);
    return () => observer.disconnect();
  });

  $effect(() => { total; clean; conflicted; escalated; render(); });
</script>

<div class="rounded-xl bg-gray-900 border border-gray-800 p-4">
  <h4 class="text-xs font-semibold text-gray-400 uppercase tracking-wider mb-2">Merge Health</h4>
  <div bind:this={containerEl} class="w-full flex justify-center">
    <svg bind:this={svgEl}></svg>
  </div>
</div>
