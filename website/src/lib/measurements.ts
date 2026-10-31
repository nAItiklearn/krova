import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';

/**
 * Reads every docs/measurements/idle-*.txt at build time. Nothing is hand-copied:
 * every number on the Performance page and in the home-page teaser is derived from
 * the parsed files. The build fails loudly if a file is missing its data or is truncated
 * (for example the output of a run that printed NOT_RUNNING).
 *
 * One file is one run of docs/measurements/measure-idle.ps1 in one state of the app
 * (tray only, editor window open, ...). Line format (see the "#" lines in the files):
 *
 *   # comment lines. "# State: ..." (any case) describes the build and the state of the app.
 *   #   "# Not measured: ..." lists what the run does not cover.
 *   HOST|cpu|cores|ram|os
 *   LABEL|short name of the state            (optional; the script's -Label)
 *   TARGET|pid|path|identifier               (optional; read but never shown: it holds a local path)
 *   COUNT|app+webview2 processes: N
 *   S|seconds|krova-app working set MB|all working set MB|all private MB|cpu % of one core[|procs N]
 *   DONE
 *
 * The date comes from the file name (idle-YYYY-MM-DD...txt). A file without a LABEL line
 * (the first measurement, 2026-10-30, predates it) gets a label read from its State line.
 *
 * KROVA_MEASUREMENTS_DIR overrides the folder. It exists so the multi-file rendering can be tested
 * against fixture files without putting them into docs/measurements/.
 */

export interface Sample {
  t: number;
  appWorkingSet: number;
  allWorkingSet: number;
  allPrivate: number;
  cpu: number;
  /** Process count at that sample, when the run recorded it (the "procs N" column). */
  procs?: number;
}

export interface Host {
  cpu: string;
  cores: string;
  ram: string;
  os: string;
}

export interface Measurement {
  file: string;
  /** YYYY-MM-DD from the file name, or '' when the name has none. */
  date: string;
  /** Short name of the state, for headings and chart rows. */
  label: string;
  /** True when the label came from a LABEL line, false when it was worked out from the State line. */
  labelFromFile: boolean;
  comments: string[];
  /** Text of the "# State:" line: which build, and what the app was doing. */
  state: string;
  notMeasured: string;
  host: Host;
  /** Processes counted at the start of the run: krova-app.exe plus its web view processes. */
  processCount: number;
  samples: Sample[];
}

const median = (xs: number[]): number => {
  const s = [...xs].sort((a, b) => a - b);
  const m = Math.floor(s.length / 2);
  return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
};

/** A readable label for a run that has no LABEL line, taken from its State line. */
function inferLabel(state: string, date: string): string {
  if (/\bwindow\b[^.]*\bopen\b/i.test(state)) return 'Editor window open';
  if (/\b(tray only|no window)\b/i.test(state) && !/\bopen\b/i.test(state)) return 'Tray only, no window';
  return date ? `Run of ${date}` : 'Unlabelled run';
}

function parseFile(dir: string, file: string): Measurement {
  const dateMatch = /^idle-(\d{4}-\d{2}-\d{2})/.exec(file);
  const date = dateMatch ? dateMatch[1] : '';
  const lines = readFileSync(resolve(dir, file), 'utf8').replace(/\r\n/g, '\n').split('\n');

  const comments: string[] = [];
  const samples: Sample[] = [];
  let host: Host | undefined;
  let label = '';
  let processCount = 0;
  let done = false;

  for (const raw of lines) {
    const line = raw.trim().replace(/^﻿/, '');
    if (!line) continue;
    if (line.startsWith('#')) {
      comments.push(line.replace(/^#\s*/, ''));
    } else if (line.startsWith('HOST|')) {
      const [, cpu, cores, ram, os] = line.split('|');
      host = { cpu, cores, ram, os };
    } else if (line.startsWith('LABEL|')) {
      label = line.slice('LABEL|'.length).trim();
    } else if (line.startsWith('COUNT|')) {
      processCount = Number(/(\d+)\s*$/.exec(line)?.[1] ?? 0);
    } else if (line.startsWith('S|')) {
      const p = line.split('|');
      const nums = p.slice(1, 6).map(Number);
      if ((p.length !== 6 && p.length !== 7) || nums.some((n) => !Number.isFinite(n))) {
        throw new Error(`Malformed sample line in ${file}: ${line}`);
      }
      const procs = p.length === 7 ? Number(/(\d+)\s*$/.exec(p[6])?.[1]) : NaN;
      samples.push({
        t: nums[0],
        appWorkingSet: nums[1],
        allWorkingSet: nums[2],
        allPrivate: nums[3],
        cpu: nums[4],
        ...(Number.isFinite(procs) ? { procs } : {}),
      });
    } else if (line === 'DONE') {
      done = true;
    }
  }
  if (!processCount && samples.length > 0 && samples.every((s) => s.procs !== undefined)) {
    processCount = median(samples.map((s) => s.procs as number));
  }
  if (!host || !processCount || samples.length === 0 || !done) {
    throw new Error(`${file} is incomplete (needs HOST, COUNT, S rows and DONE).`);
  }

  const stateLine = comments.find((c) => /^State:/i.test(c)) ?? '';
  const notMeasuredLine = comments.find((c) => /^Not measured:/i.test(c)) ?? '';
  const state = stateLine.replace(/^State:\s*/i, '');

  return {
    file,
    date,
    label: label || inferLabel(state, date),
    labelFromFile: label !== '',
    comments,
    state,
    notMeasured: notMeasuredLine.replace(/^Not measured:\s*/i, ''),
    host,
    processCount,
    samples,
  };
}

let cache: Measurement[] | undefined;

function measurementsDir(): string {
  const fromEnv = process.env.KROVA_MEASUREMENTS_DIR;
  return fromEnv ? resolve(fromEnv) : resolve(process.cwd(), '../docs/measurements');
}

/**
 * Every idle-*.txt run, newest date first; on the same date the run with fewer processes
 * comes first (tray only before window open), then by file name. At least one is required.
 */
export function loadMeasurements(): Measurement[] {
  if (cache) return cache;
  const dir = measurementsDir();
  if (!existsSync(dir)) {
    throw new Error(`Measurements folder not found: ${dir}. Run the website scripts from the website/ folder of the Krova repository.`);
  }
  const files = readdirSync(dir).filter((f) => /^idle-.+\.txt$/.test(f));
  if (files.length === 0) throw new Error(`No idle-*.txt file in ${dir}.`);

  cache = files
    .map((f) => parseFile(dir, f))
    .sort((a, b) => b.date.localeCompare(a.date) || a.processCount - b.processCount || a.file.localeCompare(b.file));
  return cache;
}

/** True when docs/measurements/measure-idle.ps1 exists in the repository. */
export function measureScriptPresent(): boolean {
  return existsSync(resolve(measurementsDir(), 'measure-idle.ps1'));
}

export interface IdleStats {
  durationS: number;
  intervalS: number;
  appWs: number;
  privateMedian: number;
  privateMin: number;
  privateMax: number;
  wsSumMin: number;
  wsSumMax: number;
  cpuMean: number;
  cpuMax: number;
  cpuNonZero: Sample[];
  /** Approximate memory outside Krova's own process: private total minus own working set. */
  outsideApprox: number;
  /** Lowest and highest per-sample process count, when the run recorded it. */
  procsRange?: [number, number];
}

export function idleStats(m: Measurement): IdleStats {
  const s = m.samples;
  const priv = s.map((x) => x.allPrivate);
  const ws = s.map((x) => x.allWorkingSet);
  const cpu = s.map((x) => x.cpu);
  const appWs = median(s.map((x) => x.appWorkingSet));
  const privateMedian = median(priv);
  const procs = s.map((x) => x.procs).filter((p): p is number => p !== undefined);
  return {
    durationS: s[s.length - 1].t,
    intervalS: s.length > 1 ? s[1].t - s[0].t : s[0].t,
    appWs,
    privateMedian,
    privateMin: Math.min(...priv),
    privateMax: Math.max(...priv),
    wsSumMin: Math.min(...ws),
    wsSumMax: Math.max(...ws),
    cpuMean: cpu.reduce((a, b) => a + b, 0) / cpu.length,
    cpuMax: Math.max(...cpu),
    cpuNonZero: s.filter((x) => x.cpu > 0),
    outsideApprox: privateMedian - appWs,
    procsRange: procs.length > 0 ? [Math.min(...procs), Math.max(...procs)] : undefined,
  };
}

/**
 * The build that ends the web view when the window is closed (DESTROY_ON_CLOSE in src-tauri/src/ui_window.rs)
 * and that registers the keyboard shortcuts and the window watcher was made after this date. A run dated on
 * or before it is of the earlier build, and the pages say so. Raise it only if a later build predates a run.
 */
export const EARLIER_BUILD_UNTIL = '2026-10-30';

export const isEarlierBuild = (iso: string): boolean => iso !== '' && iso <= EARLIER_BUILD_UNTIL;

/** "30 October 2026" from "2026-10-30"; '' when there is no date. */
export function prettyDate(iso: string): string {
  if (!iso) return '';
  return new Date(`${iso}T00:00:00Z`).toLocaleDateString('en-GB', {
    day: 'numeric',
    month: 'long',
    year: 'numeric',
    timeZone: 'UTC',
  });
}
