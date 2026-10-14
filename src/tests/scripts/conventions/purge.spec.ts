import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

const ROOT = resolve(__dirname, '../../../..');

describe('the CSS of the build', () => {
  it('keeps the states of the last run, whose classes are named at runtime', () => {
    const config = readFileSync(resolve(ROOT, 'vite.config.common.ts'), 'utf8');
    const status = readFileSync(resolve(ROOT, 'src/components/rules/RuleRunStatus.vue'), 'utf8');

    expect(status).toContain('`run_icon--${state}`');
    expect(config).toContain('/^run_icon--/');
  });
});
