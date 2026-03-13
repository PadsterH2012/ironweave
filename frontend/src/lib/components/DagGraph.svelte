<script lang="ts">
  import cytoscape from 'cytoscape';
  import type { Core } from 'cytoscape';

  interface Stage {
    id: string;
    name: string;
    runtime: string;
    prompt: string;
    depends_on: string[];
    is_manual_gate: boolean;
  }

  interface Props {
    stages: Stage[];
    stageStatuses?: Record<string, string>;
    onStageClick?: (stage: Stage, status: string | null) => void;
  }

  let { stages, stageStatuses, onStageClick }: Props = $props();

  let container: HTMLDivElement | undefined = $state();
  let cy: Core | undefined = $state();

  function getStatusColor(stageId: string): string {
    if (!stageStatuses) return '#374151';
    const status = stageStatuses[stageId];
    if (!status) return '#6b7280';
    const s = status.toLowerCase();
    if (s === 'pending') return '#6b7280';
    if (s === 'running') return '#3b82f6';
    if (s === 'waiting_approval' || s === 'waitingapproval') return '#f59e0b';
    if (s === 'completed') return '#10b981';
    if (s === 'skipped') return '#9ca3af';
    if (s === 'failed' || typeof status === 'object' || (typeof status === 'string' && status.startsWith('{'))) return '#ef4444';
    return '#6b7280';
  }

  function getStatusName(stageId: string): string | null {
    if (!stageStatuses) return null;
    const status = stageStatuses[stageId];
    if (!status) return null;
    if (typeof status === 'string' && status.startsWith('{')) {
      try {
        const parsed = JSON.parse(status);
        if (parsed.Failed) return `Failed: ${parsed.Failed}`;
      } catch { /* ignore */ }
      return 'Failed';
    }
    if (typeof status === 'object') {
      const obj = status as Record<string, string>;
      if (obj.Failed) return `Failed: ${obj.Failed}`;
      return 'Unknown';
    }
    return status;
  }

  function getBorderStyle(stageId: string): string {
    if (!stageStatuses) return 'solid';
    const status = stageStatuses[stageId];
    if (status.toLowerCase() === 'skipped') return 'dashed';
    return 'solid';
  }

  function getBorderWidth(stageId: string): number {
    if (!stageStatuses) return 2;
    const status = stageStatuses[stageId];
    if (status.toLowerCase() === 'running') return 4;
    return 2;
  }

  function isFailed(stageId: string): boolean {
    if (!stageStatuses) return false;
    const status = stageStatuses[stageId];
    if (!status) return false;
    if (typeof status === 'string' && status.startsWith('{')) return true;
    if (typeof status === 'object') return true;
    return false;
  }

  $effect(() => {
    if (!container) return;

    const elements: cytoscape.ElementDefinition[] = [];

    // Create nodes
    for (const stage of stages) {
      elements.push({
        data: {
          id: stage.id,
          label: stage.name,
          isManualGate: stage.is_manual_gate,
          color: getStatusColor(stage.id),
          borderStyle: getBorderStyle(stage.id),
          borderWidth: getBorderWidth(stage.id),
          failed: isFailed(stage.id),
        },
      });
    }

    // Create edges from dependencies
    for (const stage of stages) {
      for (const dep of stage.depends_on) {
        elements.push({
          data: {
            id: `${dep}->${stage.id}`,
            source: dep,
            target: stage.id,
          },
        });
      }
    }

    if (cy) {
      cy.destroy();
    }

    cy = cytoscape({
      container,
      elements,
      style: [
        {
          selector: 'node',
          style: {
            'background-color': 'data(color)',
            'label': 'data(label)',
            'color': '#e5e7eb',
            'text-valign': 'center',
            'text-halign': 'center',
            'font-size': '12px',
            'font-family': 'ui-sans-serif, system-ui, sans-serif',
            'width': 140,
            'height': 50,
            'shape': 'roundrectangle',
            'border-width': 'data(borderWidth)',
            'border-color': 'data(color)',
            'border-style': 'data(borderStyle)' as any,
            'text-wrap': 'wrap',
            'text-max-width': '120px',
            'padding': '8px',
          },
        },
        {
          selector: 'node[?isManualGate]',
          style: {
            'shape': 'diamond',
            'width': 80,
            'height': 80,
            'font-size': '10px',
            'text-max-width': '60px',
          },
        },
        {
          selector: 'edge',
          style: {
            'width': 2,
            'line-color': '#4b5563',
            'target-arrow-color': '#4b5563',
            'target-arrow-shape': 'triangle',
            'curve-style': 'bezier',
            'arrow-scale': 0.8,
          },
        },
      ],
      layout: {
        name: 'breadthfirst',
        directed: true,
        spacingFactor: 1.5,
        avoidOverlap: true,
        padding: 30,
      },
      userZoomingEnabled: true,
      userPanningEnabled: true,
      boxSelectionEnabled: false,
    });

    // Click handler
    cy.on('tap', 'node', (evt) => {
      const nodeId = evt.target.id();
      const stage = stages.find((s) => s.id === nodeId);
      if (stage && onStageClick) {
        const statusName = getStatusName(nodeId);
        onStageClick(stage, statusName);
      }
    });

    return () => {
      if (cy) {
        cy.destroy();
        cy = undefined;
      }
    };
  });
</script>

<div bind:this={container} class="w-full h-full min-h-[400px] bg-gray-950 rounded-xl border border-gray-800"></div>
