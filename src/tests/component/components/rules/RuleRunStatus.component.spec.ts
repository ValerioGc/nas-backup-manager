import { afterEach, expect, it } from 'vitest';
import { mount } from '@vue/test-utils';
import RuleRunStatus from '@/components/rules/RuleRunStatus.vue';
import { messages, setLocale } from '@/i18n';

afterEach(() => setLocale('it'));

it.each([
  ['ok', messages.it.runStatus.ok],
  ['failed', messages.it.runStatus.failed],
  ['interrupted', messages.it.runStatus.interrupted],
] as const)('shows a run that ended %s, with its time', async (status, label) => {
  const wrapper = mount(RuleRunStatus, {
    props: {
      running: false,
      lastRun: { at: Date.UTC(2026, 9, 10, 16, 30), status, message: null },
    },
  });
  const icon = wrapper.get(`[data-testid="run_status_${status}"]`);
  expect(icon.attributes('aria-label')).toContain(label);
  expect(icon.attributes('aria-label')).toContain('Ultima esecuzione 10/10/26');
  await icon.trigger('focus');
  expect(icon.findAll('.run_tooltip_line')).toHaveLength(2);
  await icon.trigger('blur');
  expect(icon.find('[role="tooltip"]').exists()).toBe(false);
  await icon.trigger('click');
  expect(icon.find('[role="tooltip"]').exists()).toBe(true);
  await icon.trigger('keydown', { key: 'Escape' });
  expect(icon.find('[role="tooltip"]').exists()).toBe(false);
});

it('says when a rule runs or never ran', () => {
  setLocale('en');
  const running = mount(RuleRunStatus, { props: { running: true, lastRun: null } });
  expect(running.get('[data-testid="run_status_running"]').attributes('aria-label')).toBe(
    messages.en.runStatus.running,
  );
  const never = mount(RuleRunStatus, { props: { running: false, lastRun: null } });
  expect(never.get('[data-testid="run_status_never"]').attributes('aria-label')).toBe(
    messages.en.never,
  );
});
