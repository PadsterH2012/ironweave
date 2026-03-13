<script lang="ts">
  import { Terminal } from '@xterm/xterm';
  import { FitAddon } from '@xterm/addon-fit';
  import '@xterm/xterm/css/xterm.css';

  interface Props {
    agentId: string;
  }

  let { agentId }: Props = $props();

  let containerEl: HTMLDivElement | undefined = $state();

  $effect(() => {
    if (!containerEl) return;

    const term = new Terminal({
      cursorBlink: true,
      cursorStyle: 'block',
      fontSize: 13,
      fontFamily: "'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace",
      theme: {
        background: 'transparent',
        foreground: '#e4e4e7',
        cursor: '#22c55e',
        selectionBackground: '#3f3f4640',
        black: '#18181b',
        red: '#ef4444',
        green: '#22c55e',
        yellow: '#eab308',
        blue: '#3b82f6',
        magenta: '#a855f7',
        cyan: '#06b6d4',
        white: '#e4e4e7',
      },
      allowTransparency: true,
      scrollback: 5000,
    });

    const fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    term.open(containerEl);

    // Initial fit after a frame so the container has dimensions
    requestAnimationFrame(() => {
      try { fitAddon.fit(); } catch {}
    });

    // Resize observer for responsive fitting
    const resizeObserver = new ResizeObserver(() => {
      try { fitAddon.fit(); } catch {}
    });
    resizeObserver.observe(containerEl);

    // Connect WebSocket with binary support
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const ws = new WebSocket(`${protocol}//${window.location.host}/ws/agents/${agentId}`);
    ws.binaryType = 'arraybuffer';

    ws.onmessage = (event) => {
      if (event.data instanceof ArrayBuffer) {
        // Raw PTY output
        term.write(new Uint8Array(event.data));
      } else {
        // JSON control message
        try {
          const msg = JSON.parse(event.data);
          if (msg.type === 'exit') {
            term.write(`\r\n\x1b[33m[Process exited with code ${msg.code ?? '?'}]\x1b[0m\r\n`);
          } else if (msg.type === 'error') {
            term.write(`\r\n\x1b[31m[Error: ${msg.message ?? 'unknown'}]\x1b[0m\r\n`);
          }
        } catch {
          // Non-JSON text, write as-is
          term.write(event.data);
        }
      }
    };

    // Send keyboard input as binary frames
    term.onData((data: string) => {
      if (ws.readyState === WebSocket.OPEN) {
        ws.send(new TextEncoder().encode(data));
      }
    });

    // Send resize events as JSON text frames
    term.onResize(({ cols, rows }) => {
      if (ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify({ type: 'resize', cols, rows }));
      }
    });

    ws.onerror = () => {
      term.write('\r\n\x1b[31m[WebSocket error]\x1b[0m\r\n');
    };

    ws.onclose = () => {
      term.write('\r\n\x1b[33m[Connection closed]\x1b[0m\r\n');
    };

    return () => {
      resizeObserver.disconnect();
      if (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING) {
        ws.close();
      }
      term.dispose();
    };
  });
</script>

<div
  bind:this={containerEl}
  class="terminal-container w-full h-full min-h-0"
></div>

<style>
  .terminal-container :global(.xterm) {
    height: 100%;
    padding: 4px;
  }
  .terminal-container :global(.xterm-viewport) {
    background-color: transparent !important;
  }
</style>
