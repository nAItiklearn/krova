import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';

/**
 * Reads the newest docs/measurements/idle-YYYY-MM-DD.txt at build time.
 * Nothing is hand-copied: every number on the Performance page is derived from
 * the parsed file. The build fails loudly if the file is missing or truncated.
 *
 * File format (see the "#" lines in the file itself):
 *   # comment lines (test state, column meanings)
 *   HOST|cpu|cores|ram|os
 *   COUNT|app+webview2 processes: N
 *   S|seconds|krova-app working set MB|all working set MB|all private MB|cpu % of one core
 *   DONE
 */

export interface Sample {
  t: number;
  appWorkingSet: number;
  allWorkingSet: number;
  allPrivate: number;
  cpu: number;
}

export interface Host {
  cpu: string;
  cores: string;
  ram: string;
  os: string;
}

export interface Measurement {
  file: string;
  date: string;
  comments: string[];
  state: string;
  notMeasured: string;
  host: Host;
  processCount: number;
  samples: Sample[];
  /** True when docs/measurements/measure-idle.ps1 exists in the repository. */
  scriptPresent: boolean;
}

const median = (xs: number[]): number => {
  const s = [...xs].sort((a, b) => a - b);
  const m = Math.floor(s.length / 2);
  return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
};

export function loadLatestIdle(): Measurement {
  const dir = resolve(process.cwd(), '../docs/measurements');
  if (!existsSync(dir)) {
    throw new Error(`Measurements folder not found: ${dir}. Run the website scripts from the website/ folder of the Krova repository.`);
  }
  const files = readdirSync(dir)
    .filter((f) => /^idle-\d{4}-\d{2}-\d{2}\.txt$/.test(f))
    .sort()
    .reverse();
  if (files.length === 0) throw new Error(`No idle-YYYY-MM-DD.txt file in ${dir}.`);

  const file = files[0];
  const date = file.slice('idle-'.length, -'.txt'.length);
  const lines = readFileSync(resolve(dir, file), 'utf8').replace(/\r\n/g, '\n').split('\n');

  const comments: string[] = [];
  const samples: Sample[] = [];
  let host: Host | undefined;
  let processCount = 0;
  let done = false;

  for (const raw of lines) {
    const line = raw.trim();
    if (!line) continue;
    if (line.startsWith('#')) {
      comments.push(line.replace(/^#\s*/, ''));
    } else if (line.startsWith('HOST|')) {
      const [, cpu, cores, ram, os] = line.split('|');
      host = { cpu, cores, ram, os };
    } else if (line.startsWith('COUNT|')) {
      processCount = Number(/(\d+)\s*$/.exec(line)?.[1] ?? 0);
    } else if (line.startsWith('S|')) {
      const p = line.split('|');
      const nums = p.slice(1).map(Number);
      if (p.length !== 6 || nums.some((n) => !Number.isFinite(n))) {
        throw new Error(`Malformed sample line in ${file}: ${line}`);
      }
      samples.push({ t: nums[0], appWorkingSet: nums[1], allWorkingSet: nums[2], allPrivate: nums[3], cpu: nums[4] });
    } else if (line === 'DONE') {
      done = true;
    }
  }
  if (!host || !processCount || samples.length === 0 || !done) {
    throw new Error(`${file} is incomplete (needs HOST, COUNT, S rows and DONE).`);
  }

  const stateLine = comments.find((c) => /^State:/i.test(c)) ?? '';
  const notMeasuredLine = comments.find((c) => /^Not measured:/i.test(c)) ?? '';

  return {
    file,
    date,
    comments,
    state: stateLine.replace(/^State:\s*/i, ''),
    notMeasured: notMeasuredLine.replace(/^Not measured:\s*/i, ''),
    host,
    processCount,
    samples,
    scriptPresent: existsSync(resolve(dir, 'measure-idle.ps1')),
  };
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
}

export function idleStats(m: Measurement): IdleStats {
  const s = m.samples;
  const priv = s.map((x) => x.allPrivate);
  const ws = s.map((x) => x.allWorkingSet);
  const cpu = s.map((x) => x.cpu);
  const appWs = median(s.map((x) => x.appWorkingSet));
  const privateMedian = median(priv);
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
  };
}
