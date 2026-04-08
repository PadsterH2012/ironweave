<script lang="ts">
  import { type DispatchSchedule } from '../api';

  const DAYS = ['Su', 'Mo', 'Tu', 'We', 'Th', 'Fr', 'Sa'];
  const DAYS_FULL = ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday'];
  const MONTHS_SHORT = ['J', 'F', 'M', 'A', 'M', 'J', 'J', 'A', 'S', 'O', 'N', 'D'];
  const MONTHS_FULL = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

  interface Props {
    schedule: DispatchSchedule;
    onClose: () => void;
    onSave: (cronExpression: string) => Promise<void>;
  }

  let { schedule, onClose, onSave }: Props = $props();

  let freq: 'interval' | 'daily' | 'weekly' | 'monthly' = $state('daily');
  let h = $state(0);
  let m = $state(0);
  let every = $state(30);
  let evUnit: 'min' | 'hr' = $state('min');
  let wStart = $state(0);
  let wEnd = $state(23);
  let days = $state<number[]>([]);
  let doms = $state<number[]>([1]);
  let months = $state<number[]>([]);
  let dirty = $state(false);
  let showNext = $state(false);
  let saving = $state(false);
  let saved = $state(false);

  function parseCron(cron: string) {
    try {
      const [mn, hr, dm, mo, dw] = cron.split(' ');
      if (mn.startsWith('*/')) {
        freq = 'interval'; every = +mn.slice(2); evUnit = 'min';
        if (hr.includes(',')) {
          const segs = hr.split(',');
          const first = segs[0].split('-');
          const last = segs[segs.length - 1].split('-');
          wStart = +first[0]; wEnd = +(last[1] || last[0]);
        } else if (hr.includes('-')) {
          const [s, e] = hr.split('-').map(Number); wStart = s; wEnd = e;
        }
      } else if (hr.startsWith('*/')) {
        freq = 'interval'; every = +hr.slice(2); evUnit = 'hr'; m = +mn;
      } else {
        if (dm !== '*') { freq = 'monthly'; doms = dm.split(',').map(Number); }
        else if (dw !== '*' && dw.split(',').length <= 2) freq = 'weekly';
        else freq = 'daily';
        h = hr === '*' ? 0 : +hr;
        m = mn === '*' ? 0 : +mn;
      }
      days = dw !== '*'
        ? (dw.includes('-')
          ? Array.from({ length: +dw.split('-')[1] - +dw.split('-')[0] + 1 }, (_, i) => i + +dw.split('-')[0])
          : dw.split(',').map(Number))
        : [];
      months = mo !== '*' ? mo.split(',').map(Number) : [];
    } catch {}
  }

  // Parse on creation — runs synchronously before first render
  parseCron(schedule.cron_expression);

  function buildCron(): string {
    let mn = '*', hr = '*', dm = '*', mo = '*', dw = '*';
    if (freq === 'interval') {
      if (evUnit === 'min') {
        mn = `*/${every}`;
        hr = wStart <= wEnd ? `${wStart}-${wEnd}` : `${wStart}-23,0-${wEnd}`;
      } else {
        mn = String(m); hr = `*/${every}`;
      }
      if (days.length > 0 && days.length < 7) dw = days.join(',');
    } else if (freq === 'daily') {
      mn = String(m); hr = String(h);
      if (days.length > 0 && days.length < 7) dw = days.join(',');
    } else if (freq === 'weekly') {
      mn = String(m); hr = String(h); dw = days.length ? days.join(',') : '1';
    } else if (freq === 'monthly') {
      mn = String(m); hr = String(h); dm = doms.length ? doms.join(',') : '1';
    }
    if (months.length > 0 && months.length < 12) mo = months.join(',');
    return `${mn} ${hr} ${dm} ${mo} ${dw}`;
  }

  let expr = $derived(buildCron());

  function matchField(f: string, v: number): boolean {
    if (f === '*') return true;
    return f.split(',').some(p => {
      if (p.includes('/')) {
        const [r, s] = p.split('/');
        const st = +s;
        if (r === '*') return v % st === 0;
        const [a, b] = r.split('-').map(Number);
        return v >= a && v <= b && (v - a) % st === 0;
      }
      if (p.includes('-')) {
        const [a, b] = p.split('-').map(Number);
        return v >= a && v <= b;
      }
      return +p === v;
    });
  }

  let runs = $derived.by(() => {
    try {
      const [mF, hF, dF, moF, dwF] = expr.split(' ');
      const result: Date[] = [];
      const c = new Date();
      c.setSeconds(0); c.setMilliseconds(0);
      c.setMinutes(c.getMinutes() + 1);
      let i = 0;
      while (result.length < 5 && i < 525600) {
        i++;
        if (matchField(mF, c.getMinutes()) && matchField(hF, c.getHours()) &&
            matchField(dF, c.getDate()) && matchField(moF, c.getMonth() + 1) &&
            matchField(dwF, c.getDay())) {
          result.push(new Date(c));
        }
        c.setMinutes(c.getMinutes() + 1);
      }
      return result;
    } catch {
      return [];
    }
  });

  let summary = $derived.by(() => {
    const parts: string[] = [];
    if (freq === 'interval') {
      const unit = evUnit === 'min'
        ? (every === 1 ? 'minute' : `${every} minutes`)
        : (every === 1 ? 'hour' : `${every} hours`);
      parts.push(`Every ${unit}`);
      if (evUnit === 'min') parts.push(`between ${pad(wStart)}:00 – ${pad(wEnd)}:59`);
    } else {
      parts.push(`Runs at ${pad(h)}:${pad(m)}`);
    }
    if (freq === 'monthly') {
      if (doms.length > 0) {
        const ordinal = (n: number) => {
          const s = ['th', 'st', 'nd', 'rd'];
          const v = n % 100;
          return n + (s[(v - 20) % 10] || s[v] || s[0]);
        };
        parts.push(`on the ${doms.map(ordinal).join(', ')} of each month`);
      }
    } else if (days.length === 7 || days.length === 0) {
      parts.push('every day');
    } else if (days.length === 5 && [1, 2, 3, 4, 5].every(d => days.includes(d))) {
      parts.push('weekdays only');
    } else if (days.length === 2 && days.includes(0) && days.includes(6)) {
      parts.push('weekends only');
    } else {
      parts.push(days.map(d => DAYS_FULL[d]).join(', '));
    }
    if (months.length > 0 && months.length < 12) {
      parts.push(`in ${months.map(mo => MONTHS_FULL[mo - 1]).join(', ')}`);
    }
    return parts;
  });

  function toggleArr(arr: number[], val: number): number[] {
    return arr.includes(val) ? arr.filter(x => x !== val) : [...arr, val].sort((a, b) => a - b);
  }

  function mark() { dirty = true; saved = false; }

  function pad(n: number) { return String(n).padStart(2, '0'); }

  async function handleSave() {
    saving = true;
    try {
      await onSave(expr);
      dirty = false;
      saved = true;
      setTimeout(() => { saved = false; }, 2000);
    } finally {
      saving = false;
    }
  }
</script>

<!-- Backdrop -->
<div
  class="fixed inset-0 bg-black/50 z-40"
  role="presentation"
  onclick={onClose}
></div>

<!-- Modal -->
<div class="fixed inset-0 z-50 flex items-start justify-center pt-20 pointer-events-none">
  <div class="bg-gray-900 border border-gray-800 rounded-lg shadow-2xl w-[360px] overflow-hidden pointer-events-auto">

    <!-- Header -->
    <div class="flex items-center justify-between px-3.5 py-2.5 border-b border-gray-800">
      <div class="flex items-center gap-2 min-w-0">
        <span class="text-xs font-bold text-gray-100 font-mono flex-shrink-0">Schedule</span>
        <span class="text-gray-600 text-xs flex-shrink-0">·</span>
        <span class="text-xs text-purple-400 font-mono truncate">{schedule.description ?? schedule.cron_expression}</span>
      </div>
      <button onclick={onClose} class="text-gray-600 hover:text-gray-300 text-lg leading-none transition-colors ml-2 flex-shrink-0">&times;</button>
    </div>

    <div class="px-3.5 py-3 flex flex-col gap-3">

      <!-- Frequency -->
      <div class="flex items-center gap-1.5">
        {#each (['interval', 'daily', 'weekly', 'monthly'] as const) as f}
          <button
            onclick={() => { freq = f; mark(); }}
            class="h-[26px] px-2 inline-flex items-center justify-center text-[10px] font-mono rounded-sm border transition-all
                   {freq === f ? 'bg-purple-500 text-gray-900 border-purple-500 font-bold' : 'bg-transparent text-gray-600 border-gray-700 hover:border-gray-500'}"
          >{f === 'interval' ? '⟳' : f === 'daily' ? 'D' : f === 'weekly' ? 'W' : 'M'}</button>
        {/each}
        <span class="text-[10px] text-gray-600 font-mono ml-1">{freq}</span>
      </div>

      <!-- Time / Interval controls -->
      {#if freq === 'interval'}
        <div class="flex flex-col gap-1.5">
          <div class="flex items-center gap-1.5 text-[11px] text-gray-500">
            <span>every</span>
            <input type="number" min="1" max="60" bind:value={every} onchange={mark}
              class="w-9 text-center text-xs font-mono bg-gray-950 border border-gray-700 rounded-sm px-1 py-0.5 text-gray-100 focus:outline-none focus:border-gray-500"
            />
            {#each (['min', 'hr'] as const) as u}
              <button
                onclick={() => { evUnit = u; mark(); }}
                class="h-[26px] px-2 text-[10px] font-mono rounded-sm border transition-all
                       {evUnit === u ? 'bg-purple-500 text-gray-900 border-purple-500 font-bold' : 'bg-transparent text-gray-600 border-gray-700 hover:border-gray-500'}"
              >{u}</button>
            {/each}
          </div>
          {#if evUnit === 'min'}
            <div class="flex items-center gap-1.5 text-[11px] text-gray-500">
              <span>between</span>
              <input type="number" min="0" max="23" bind:value={wStart} onchange={mark}
                class="w-8 text-center text-xs font-mono bg-gray-950 border border-gray-700 rounded-sm px-1 py-0.5 text-gray-100 focus:outline-none focus:border-gray-500"
              />
              <span class="text-gray-600">→</span>
              <input type="number" min="0" max="23" bind:value={wEnd} onchange={mark}
                class="w-8 text-center text-xs font-mono bg-gray-950 border border-gray-700 rounded-sm px-1 py-0.5 text-gray-100 focus:outline-none focus:border-gray-500"
              />
              <span>hrs</span>
            </div>
          {:else}
            <div class="flex items-center gap-1.5 text-[11px] text-gray-500">
              <span>at minute</span>
              <input type="number" min="0" max="59" bind:value={m} onchange={mark}
                class="w-9 text-center text-xs font-mono bg-gray-950 border border-gray-700 rounded-sm px-1 py-0.5 text-gray-100 focus:outline-none focus:border-gray-500"
              />
            </div>
          {/if}
        </div>
      {:else}
        <div class="flex items-center gap-1.5 text-[11px] text-gray-500">
          <span>at</span>
          <input type="number" min="0" max="23" bind:value={h} onchange={mark}
            class="w-8 text-center text-xs font-mono bg-gray-950 border border-gray-700 rounded-sm px-1 py-0.5 text-gray-100 focus:outline-none focus:border-gray-500"
          />
          <span class="text-gray-600 font-bold">:</span>
          <input type="number" min="0" max="59" bind:value={m} onchange={mark}
            class="w-8 text-center text-xs font-mono bg-gray-950 border border-gray-700 rounded-sm px-1 py-0.5 text-gray-100 focus:outline-none focus:border-gray-500"
          />
        </div>
      {/if}

      <!-- Days of week -->
      {#if freq !== 'monthly'}
        <div class="flex gap-1">
          {#each DAYS as d, i}
            <button
              onclick={() => { days = toggleArr(days, i); mark(); }}
              class="w-7 h-[26px] inline-flex items-center justify-center text-[10px] font-mono rounded-sm border transition-all
                     {days.includes(i) ? 'bg-purple-500 text-gray-900 border-purple-500 font-bold' : 'bg-transparent text-gray-600 border-gray-700 hover:border-gray-500'}"
            >{d}</button>
          {/each}
        </div>
      {/if}

      <!-- Days of month -->
      {#if freq === 'monthly'}
        <div>
          <div class="text-[10px] text-gray-600 font-mono mb-1">day of month</div>
          <div class="flex flex-wrap gap-1 max-w-[320px]">
            {#each Array.from({length: 31}, (_, i) => i + 1) as d}
              <button
                onclick={() => { doms = toggleArr(doms, d); mark(); }}
                class="w-6 h-[22px] inline-flex items-center justify-center text-[9px] font-mono rounded-sm border transition-all
                       {doms.includes(d) ? 'bg-purple-500 text-gray-900 border-purple-500 font-bold' : 'bg-transparent text-gray-600 border-gray-700 hover:border-gray-500'}"
              >{d}</button>
            {/each}
          </div>
        </div>
      {/if}

      <!-- Months -->
      <div>
        <div class="text-[10px] text-gray-600 font-mono mb-1">months <span class="text-gray-700">(blank = all)</span></div>
        <div class="flex gap-1">
          {#each MONTHS_SHORT as ml, i}
            <button
              onclick={() => { months = toggleArr(months, i + 1); mark(); }}
              class="w-6 h-[26px] inline-flex items-center justify-center text-[10px] font-mono rounded-sm border transition-all
                     {months.includes(i + 1) ? 'bg-purple-500 text-gray-900 border-purple-500 font-bold' : 'bg-transparent text-gray-600 border-gray-700 hover:border-gray-500'}"
            >{ml}</button>
          {/each}
        </div>
      </div>
    </div>

    <!-- Summary -->
    <div class="px-3.5 pb-3 flex flex-col gap-2">
      <div class="bg-green-500/5 border border-green-500/15 rounded-md px-3 py-2.5">
        {#each summary as line, i}
          <div class="font-mono leading-relaxed {i === 0 ? 'text-[13px] font-bold text-green-400' : 'text-xs font-medium text-gray-300'}">{line}</div>
        {/each}

        {#if freq === 'interval' && evUnit === 'min'}
          <div class="mt-2 px-2 py-1.5 bg-amber-500/10 border border-amber-500/20 rounded text-[11px] font-mono text-amber-400 font-semibold flex items-center gap-2">
            <span>🕐</span>
            Active: {pad(wStart)}:00 – {pad(wEnd)}:59
            {#if wStart > wEnd}<span class="text-gray-500 font-normal"> (overnight)</span>{/if}
          </div>
        {/if}

        <div class="mt-2 pt-2 border-t border-green-500/10 flex items-center gap-2 text-[11px] font-mono">
          <span class="text-gray-600">cron</span>
          <code class="text-green-400 font-semibold tracking-wide">{expr}</code>
        </div>
      </div>

      <!-- Next runs -->
      <button
        onclick={() => { showNext = !showNext; }}
        class="flex items-center gap-1.5 text-[10px] font-mono text-gray-600 hover:text-gray-400 transition-colors text-left"
      >
        <span class="transition-transform {showNext ? 'rotate-90' : ''} inline-block">▸</span>
        next {runs.length} runs
      </button>

      {#if showNext}
        <div class="flex flex-col gap-0.5 pl-3">
          {#each runs as d, i}
            <div class="flex items-center gap-1.5 text-[10px] font-mono {i === 0 ? 'text-purple-400' : 'text-gray-600'}">
              <span class="w-1 h-1 rounded-full flex-shrink-0 {i === 0 ? 'bg-purple-400' : 'bg-gray-700'}"></span>
              {d.toLocaleString('en-GB', { weekday: 'short', day: '2-digit', month: 'short', hour: '2-digit', minute: '2-digit', hour12: false })}
            </div>
          {/each}
        </div>
      {/if}
    </div>

    <!-- Footer -->
    <div class="px-3.5 py-2.5 border-t border-gray-800 flex justify-end gap-2">
      <button
        onclick={onClose}
        class="px-3.5 py-1.5 text-[11px] font-mono bg-transparent border border-gray-700 rounded text-gray-500 hover:text-gray-300 hover:border-gray-600 transition-colors"
      >Cancel</button>
      <button
        onclick={handleSave}
        disabled={!dirty || saving}
        class="px-3.5 py-1.5 text-[11px] font-mono font-bold rounded border transition-all
               {saved ? 'bg-green-500/15 border-green-500 text-green-400' : dirty && !saving ? 'bg-purple-600 border-purple-600 text-white hover:bg-purple-500' : 'bg-gray-800/50 border-gray-700 text-gray-600 cursor-default'}"
      >{saved ? '✓ Saved' : saving ? '...' : 'Save'}</button>
    </div>

  </div>
</div>
