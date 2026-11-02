/**
 * CBC-PRAMAAN GeM Simulator -- entry point.
 *
 * Technical Implementation Specification Part 8.4. Generates realistic Government
 * e-Marketplace procurement records and drives the six trigger-point API routes in the
 * sequence a real GeM integration would, across all twelve decision pathways.
 *
 *   npx tsx simulate.ts --all
 *   npx tsx simulate.ts --scenario=P7
 *   npx tsx simulate.ts --all --api=http://localhost:3000 --seed=20260406
 *   npx tsx simulate.ts --all --dry-run --show-records
 *
 * Exit codes: 0 every scenario passed, 1 at least one failed, 2 the run could not start
 * (bad arguments, or the API could not be reached).
 */

import { ApiUnreachableError, PramaanClient, type ApiResponse } from './api';
import { evaluate, type CheckResult } from './checks';
import { DEFAULT_AS_OF, describeBid, renderBidDocument, renderVendor } from './generate';
import { loadMinistries } from './gem-data';
import { summarise } from './json';
import { buildScenario, type Scenario, type Step } from './scenarios';
import { ALL_PATHWAYS, type PathwayId } from './types';

// ---------------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------------

interface Options {
  scenarios: PathwayId[];
  baseUrl: string;
  seed: number;
  runId: string;
  asOf: string;
  dryRun: boolean;
  showRecords: boolean;
  timeoutMs: number;
  json: boolean;
  colour: boolean;
}

const USAGE = `
CBC-PRAMAAN GeM Simulator

  Generates realistic GeM procurement records and drives the six trigger-point routes
  across the twelve decision pathways, asserting the on-chain answer each pathway
  requires. Exits non-zero if any scenario fails.

Usage
  npx tsx simulate.ts [options]

Options
  --all                    Run all twelve pathway scenarios and print a summary table.
  --scenario=P7            Run one pathway. Repeatable, or comma-separated: --scenario=P8,P9
  --api=URL                Base URL of the Next.js app. Default http://localhost:3000
  --seed=N                 Seed for record generation. Same seed, same records. Default 20260406
  --run-id=TOKEN           Disambiguates certificate ids between runs. Default: derived from the clock
  --as-of=YYYY-MM-DD       The simulated "today" that bid dates hang off. Default ${DEFAULT_AS_OF}
  --timeout=MS             Per-request timeout. Default 20000 (the routes budget 10s for finality)
  --dry-run                Print the request sequence without calling anything
  --show-records           Print the generated GeM bid documents and seller profiles
  --json                   Emit a machine-readable summary on stdout instead of the table
  --no-colour              Disable ANSI colour
  -h, --help               This message

Examples
  npx tsx simulate.ts --all
  npx tsx simulate.ts --scenario=P10 --show-records
  npx tsx simulate.ts --all --dry-run > demo-script.txt
`;

function parseArgs(argv: string[]): Options | 'help' {
  const options: Options = {
    scenarios: [],
    baseUrl: 'http://localhost:3000',
    seed: 20260406,
    runId: defaultRunId(),
    asOf: DEFAULT_AS_OF,
    dryRun: false,
    showRecords: false,
    timeoutMs: 20_000,
    json: false,
    colour: process.stdout.isTTY === true && !process.env.NO_COLOR,
  };

  for (const arg of argv) {
    if (arg === '-h' || arg === '--help') return 'help';
    if (arg === '--all') {
      options.scenarios = [...ALL_PATHWAYS];
      continue;
    }
    if (arg === '--dry-run') {
      options.dryRun = true;
      continue;
    }
    if (arg === '--show-records') {
      options.showRecords = true;
      continue;
    }
    if (arg === '--json') {
      options.json = true;
      continue;
    }
    if (arg === '--no-colour' || arg === '--no-color') {
      options.colour = false;
      continue;
    }

    const match = /^--([a-z-]+)=(.*)$/.exec(arg);
    if (!match) {
      throw new UsageError(`Unrecognised argument "${arg}". Run with --help for usage.`);
    }
    const key = match[1] as string;
    const value = match[2] as string;

    switch (key) {
      case 'scenario': {
        for (const part of value.split(',')) {
          const id = part.trim().toUpperCase();
          if (!ALL_PATHWAYS.includes(id as PathwayId)) {
            throw new UsageError(
              `Unknown pathway "${part.trim()}". Valid pathways: ${ALL_PATHWAYS.join(', ')}`,
            );
          }
          if (!options.scenarios.includes(id as PathwayId)) options.scenarios.push(id as PathwayId);
        }
        break;
      }
      case 'api':
        if (!/^https?:\/\//.test(value)) {
          throw new UsageError(`--api must be an http(s) URL, got "${value}"`);
        }
        options.baseUrl = value;
        break;
      case 'seed': {
        const seed = Number(value);
        if (!Number.isInteger(seed) || seed < 0) {
          throw new UsageError(`--seed must be a non-negative integer, got "${value}"`);
        }
        options.seed = seed;
        break;
      }
      case 'run-id':
        if (!/^[A-Za-z0-9._-]{1,24}$/.test(value)) {
          throw new UsageError(`--run-id must be 1-24 characters of [A-Za-z0-9._-], got "${value}"`);
        }
        options.runId = value;
        break;
      case 'as-of':
        if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) {
          throw new UsageError(`--as-of must be YYYY-MM-DD, got "${value}"`);
        }
        options.asOf = value;
        break;
      case 'timeout': {
        const ms = Number(value);
        if (!Number.isInteger(ms) || ms < 1_000) {
          throw new UsageError(`--timeout must be an integer of at least 1000 ms, got "${value}"`);
        }
        options.timeoutMs = ms;
        break;
      }
      default:
        throw new UsageError(`Unrecognised option "--${key}". Run with --help for usage.`);
    }
  }

  if (options.scenarios.length === 0) {
    throw new UsageError('Nothing to run. Pass --all, or --scenario=P1 (see --help).');
  }
  // Keep the canonical P1..P12 order regardless of the order given on the command line.
  options.scenarios.sort((a, b) => ALL_PATHWAYS.indexOf(a) - ALL_PATHWAYS.indexOf(b));
  return options;
}

class UsageError extends Error {}

function defaultRunId(): string {
  // Certificate ids must differ between runs -- the certification pallet rejects a
  // repeat with CertificateIdAlreadyUsed -- so the default is clock-derived. Pass
  // --run-id to pin it and make a whole run byte-for-byte reproducible.
  return Date.now().toString(36).slice(-6);
}

// ---------------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------------

class Printer {
  constructor(private readonly colour: boolean) {}

  private wrap(code: string, text: string): string {
    return this.colour ? `[${code}m${text}[0m` : text;
  }

  bold(text: string): string {
    return this.wrap('1', text);
  }
  green(text: string): string {
    return this.wrap('32', text);
  }
  red(text: string): string {
    return this.wrap('31', text);
  }
  yellow(text: string): string {
    return this.wrap('33', text);
  }
  dim(text: string): string {
    return this.wrap('2', text);
  }

  line(text = ''): void {
    process.stdout.write(`${text}\n`);
  }

  rule(char = '-', width = 78): void {
    this.line(this.dim(char.repeat(width)));
  }
}

// ---------------------------------------------------------------------------------
// Run
// ---------------------------------------------------------------------------------

interface StepReport {
  label: string;
  route: string;
  httpStatus: number | null;
  latencyMs: number;
  checks: CheckResult[];
  skipped: boolean;
  transportError: string | null;
}

interface ScenarioReport {
  id: PathwayId;
  title: string;
  proves: string;
  steps: StepReport[];
  passedChecks: number;
  totalChecks: number;
  passed: boolean;
  skipped: boolean;
}

async function runStep(client: PramaanClient, step: Step): Promise<StepReport> {
  if (client.isDryRun) {
    return {
      label: step.label,
      route: step.call.route,
      httpStatus: null,
      latencyMs: 0,
      checks: [],
      skipped: true,
      transportError: null,
    };
  }

  let response: ApiResponse;
  try {
    response = await client.call(step.call);
  } catch (err) {
    if (err instanceof ApiUnreachableError) throw err;
    return {
      label: step.label,
      route: step.call.route,
      httpStatus: null,
      latencyMs: 0,
      checks: step.expect.map((expectation) => ({
        passed: false,
        why: expectation.why,
        detail: `the request never completed: ${(err as Error).message}`,
      })),
      skipped: false,
      transportError: (err as Error).message,
    };
  }

  return {
    label: step.label,
    route: step.call.route,
    httpStatus: response.status,
    latencyMs: response.latencyMs,
    checks: step.expect.map((expectation) => evaluate(expectation, response)),
    skipped: false,
    transportError: null,
    // The response summary is folded into the check details, so it is not carried here.
  };
}

async function runScenario(
  client: PramaanClient,
  scenario: Scenario,
  printer: Printer,
  options: Options,
): Promise<ScenarioReport> {
  printer.line();
  printer.rule('=');
  printer.line(printer.bold(`${scenario.id}  ${scenario.title}`));
  printer.rule('=');
  printer.line(printer.dim(`Pathway   : ${scenario.pathwayText}`));
  printer.line(printer.dim(`Proves    : ${scenario.proves}`));
  printer.line();
  for (const bid of scenario.bids) {
    printer.line(printer.dim(`  bid      ${describeBid(bid)}`));
  }
  for (const vendor of scenario.vendors) {
    printer.line(
      printer.dim(
        `  seller   ${vendor.legalName} | GSTIN ${vendor.gstin} | ` +
          `${vendor.msmeCategory}${vendor.isMse ? ' (MSE)' : ''} | ${vendor.city}`,
      ),
    );
  }

  if (options.showRecords) {
    for (const bid of scenario.bids) {
      printer.line();
      printer.line(printer.dim('  --- GeM Bid Document ---'));
      for (const row of renderBidDocument(bid)) printer.line(printer.dim(`  ${row}`));
    }
    for (const vendor of scenario.vendors) {
      printer.line();
      printer.line(printer.dim('  --- GeM Seller ---'));
      for (const row of renderVendor(vendor)) printer.line(printer.dim(`  ${row}`));
    }
  }

  const steps: StepReport[] = [];
  for (const [index, step] of scenario.steps.entries()) {
    printer.line();
    printer.line(`  ${printer.bold(`Step ${index + 1}`)}  ${step.label}`);
    printer.line(printer.dim(`          ${step.narrate}`));
    printer.line(printer.dim(`          POST ${client.urlFor(step.call.route)}`));
    if (client.isDryRun) {
      printer.line(printer.dim(`          body ${summarise(step.call.body, 400)}`));
      for (const expectation of step.expect) {
        printer.line(printer.dim(`          expect  ${expectation.kind} -- ${expectation.why}`));
      }
    }

    const report = await runStep(client, step);
    steps.push(report);

    if (report.skipped) {
      printer.line(printer.yellow('          SKIPPED (dry run)'));
      continue;
    }
    if (report.transportError) {
      printer.line(printer.red(`          REQUEST FAILED: ${report.transportError}`));
    } else {
      printer.line(
        printer.dim(`          HTTP ${report.httpStatus} in ${report.latencyMs} ms`),
      );
    }
    for (const check of report.checks) {
      const mark = check.passed ? printer.green('PASS') : printer.red('FAIL');
      printer.line(`          ${mark}  ${check.why}`);
      printer.line(printer.dim(`                ${check.detail}`));
    }
  }

  const allChecks = steps.flatMap((s) => s.checks);
  const passedChecks = allChecks.filter((c) => c.passed).length;
  const skipped = steps.every((s) => s.skipped);

  return {
    id: scenario.id,
    title: scenario.title,
    proves: scenario.proves,
    steps,
    passedChecks,
    totalChecks: allChecks.length,
    passed: !skipped && allChecks.length > 0 && passedChecks === allChecks.length,
    skipped,
  };
}

function printSummary(reports: ScenarioReport[], printer: Printer, options: Options): void {
  printer.line();
  printer.rule('=');
  printer.line(printer.bold('  Summary'));
  printer.rule('=');
  printer.line(
    printer.bold(
      `  ${'Path'.padEnd(5)}${'Result'.padEnd(10)}${'Checks'.padEnd(10)}${'Steps'.padEnd(7)}Scenario`,
    ),
  );
  printer.rule();

  for (const report of reports) {
    const verdict = report.skipped
      ? printer.yellow('SKIPPED'.padEnd(10))
      : report.passed
        ? printer.green('PASS'.padEnd(10))
        : printer.red('FAIL'.padEnd(10));
    const checks = report.skipped ? '-' : `${report.passedChecks}/${report.totalChecks}`;
    printer.line(
      `  ${report.id.padEnd(5)}${verdict}${checks.padEnd(10)}${String(report.steps.length).padEnd(7)}${report.title}`,
    );
  }

  printer.rule();
  const passed = reports.filter((r) => r.passed).length;
  const failed = reports.filter((r) => !r.passed && !r.skipped).length;
  const skipped = reports.filter((r) => r.skipped).length;

  if (skipped === reports.length) {
    printer.line(
      printer.yellow(`  Dry run: ${skipped} scenario(s) planned, nothing was sent to ${options.baseUrl}.`),
    );
  } else if (failed === 0) {
    printer.line(printer.green(`  ${passed} of ${reports.length} pathways pass.`));
  } else {
    printer.line(printer.red(`  ${failed} of ${reports.length} pathways FAILED (${passed} passed).`));
    printer.line();
    for (const report of reports.filter((r) => !r.passed && !r.skipped)) {
      printer.line(printer.red(`  ${report.id} failed:`));
      for (const step of report.steps) {
        for (const check of step.checks.filter((c) => !c.passed)) {
          printer.line(`    - [${step.label}] ${check.why}`);
          printer.line(printer.dim(`      ${check.detail}`));
        }
      }
    }
  }
  printer.line();
}

async function main(): Promise<number> {
  let options: Options | 'help';
  try {
    options = parseArgs(process.argv.slice(2));
  } catch (err) {
    if (err instanceof UsageError) {
      process.stderr.write(`${err.message}\n`);
      return 2;
    }
    throw err;
  }
  if (options === 'help') {
    process.stdout.write(`${USAGE}\n`);
    return 0;
  }

  const printer = new Printer(options.colour);

  // Fail fast and clearly if the ministry list is missing or has been edited.
  const ministries = loadMinistries();

  printer.line(printer.bold('CBC-PRAMAAN GeM Simulator'));
  printer.line(
    printer.dim(
      `  API ${options.baseUrl} | seed ${options.seed} | run-id ${options.runId} | ` +
        `as-of ${options.asOf} | ${ministries.length} nodal ministries loaded`,
    ),
  );
  printer.line(
    printer.dim(`  Pathways: ${options.scenarios.join(', ')}${options.dryRun ? ' (dry run)' : ''}`),
  );

  const client = new PramaanClient({
    baseUrl: options.baseUrl,
    dryRun: options.dryRun,
    timeoutMs: options.timeoutMs,
  });

  const reports: ScenarioReport[] = [];
  for (const id of options.scenarios) {
    const scenario = buildScenario(id, {
      seed: options.seed,
      runId: options.runId,
      asOf: options.asOf,
    });
    try {
      reports.push(await runScenario(client, scenario, printer, options));
    } catch (err) {
      if (err instanceof ApiUnreachableError) {
        printer.line();
        process.stderr.write(`${err.message}\n`);
        return 2;
      }
      throw err;
    }
  }

  if (options.json) {
    process.stdout.write(
      `${JSON.stringify(
        {
          api: options.baseUrl,
          seed: options.seed,
          runId: options.runId,
          asOf: options.asOf,
          dryRun: options.dryRun,
          scenarios: reports.map((r) => ({
            pathway: r.id,
            title: r.title,
            passed: r.passed,
            skipped: r.skipped,
            checksPassed: r.passedChecks,
            checksTotal: r.totalChecks,
            failures: r.steps.flatMap((s) =>
              s.checks.filter((c) => !c.passed).map((c) => ({ step: s.label, why: c.why, detail: c.detail })),
            ),
          })),
        },
        null,
        2,
      )}\n`,
    );
  } else {
    printSummary(reports, printer, options);
  }

  if (reports.every((r) => r.skipped)) return 0;
  return reports.some((r) => !r.passed && !r.skipped) ? 1 : 0;
}

main()
  .then((code) => {
    process.exitCode = code;
  })
  .catch((err: unknown) => {
    process.stderr.write(
      `\nThe simulator stopped with an unexpected error:\n  ${
        err instanceof Error ? `${err.name}: ${err.message}` : String(err)
      }\n`,
    );
    if (err instanceof Error && err.stack) {
      process.stderr.write(`${err.stack.split('\n').slice(1).join('\n')}\n`);
    }
    process.exitCode = 2;
  });
