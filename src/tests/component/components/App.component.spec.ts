import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import App from '@/App.vue';
import cloudIcon from '@/assets/icons/cloud.svg?url';
import realtimeIcon from '@/assets/icons/realtime.svg?url';
import scheduledIcon from '@/assets/icons/scheduled.svg?url';
import { messages, setLocale } from '@/i18n';
import {
  deleteBackupRule,
  fetchBackupRules,
  runBackupRule,
  saveBackupRule,
  stopBackupRule,
  switchBackupRule,
} from '@/services/backupRules';
import { requestNetworkDisks } from '@/services/nasShell';
import type { ListedRule, SectionCode } from '@/types/backup';
import { stubDialog } from '@/tests/component/helpers/dialog';
import { answerPickers, chooseFolders, networkShare } from '@/tests/component/helpers/folders';
import { answerPowerSchedule, chooseSchedule } from '@/tests/component/helpers/schedule';

const icons: Record<SectionCode, string> = {
  scheduled: scheduledIcon,
  cloud: cloudIcon,
  rtl: realtimeIcon,
};

vi.mock('@/services/backupRules', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/services/backupRules')>()),
  fetchBackupRules: vi.fn().mockResolvedValue([]),
  saveBackupRule: vi.fn(async (rule: object) => rule),
  switchBackupRule: vi.fn().mockResolvedValue(undefined),
  runBackupRule: vi.fn().mockResolvedValue(undefined),
  stopBackupRule: vi.fn().mockResolvedValue(undefined),
  deleteBackupRule: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('@/services/nasShell', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/services/nasShell')>()),
  pickNasFolder: vi.fn(),
  requestNetworkDisks: vi.fn(),
  requestPowerSchedule: vi.fn(),
}));

let wrapper: VueWrapper | undefined;
beforeEach(() => {
  stubDialog();
  vi.clearAllMocks();
  answerPickers();
  answerPowerSchedule();
  setLocale('en');
});
afterEach(() => {
  wrapper?.unmount();
  vi.restoreAllMocks();
  setLocale('it');
});

it('renders the backup sections with an empty list', async () => {
  wrapper = mount(App);
  await flushPromises();
  const sections = wrapper.findAll('nav button').map((item) => item.text());
  expect(sections).toEqual([
    messages.en.sections.scheduled,
    messages.en.sections.rtl,
    messages.en.sections.cloud,
  ]);
  expect(wrapper.get('h1').text()).toBe(messages.en.sections.scheduled);
  expect(wrapper.find('.empty_state h2').text()).toBe(messages.en.emptySections.scheduled);
  expect(wrapper.find('.empty_state p').text()).toBe(messages.en.emptyDescription);
  expect(wrapper.findAll('th')).toHaveLength(0);
  expect(wrapper.find('dialog').exists()).toBe(false);
});

it.each<SectionCode>(['scheduled', 'cloud', 'rtl'])(
  'shows the empty state for the %s section',
  async (section) => {
    wrapper = mount(App);
    await flushPromises();
    await wrapper
      .findAll('nav button')
      .find((item) => item.text() === messages.en.sections[section])!
      .trigger('click');

    expect(wrapper.find('.empty_state h2').text()).toBe(messages.en.emptySections[section]);
    expect(wrapper.get('.empty_icon').attributes('src')).toBe(icons[section]);
  },
);

it('shows a styled backend error and retries loading rules', async () => {
  vi.mocked(fetchBackupRules).mockRejectedValueOnce(new Error('Application backend unavailable'));
  wrapper = mount(App);
  await flushPromises();

  expect(wrapper.get('.error_state').attributes('role')).toBe('alert');
  expect(wrapper.get('.error_state h2').text()).toBe(messages.en.errors.loadTitle);
  expect(wrapper.get('.error_message').text()).toBe('Application backend unavailable');

  await wrapper.get('.retry_button').trigger('click');
  await flushPromises();
  expect(wrapper.find('.error_state').exists()).toBe(false);
  expect(wrapper.find('.empty_state').exists()).toBe(true);
});

it('changes sections without confirmation when the rule has not been edited', async () => {
  wrapper = mount(App);
  await flushPromises();
  await wrapper.get('.add_button').trigger('click');

  await wrapper
    .findAll('nav button')
    .find((item) => item.text() === messages.en.sections.cloud)!
    .trigger('click');

  expect(wrapper.find('.nas_confirm_dialog').exists()).toBe(false);
  expect(wrapper.find('.rule').exists()).toBe(false);
  expect(wrapper.get('h1').text()).toBe(messages.en.sections.cloud);
});

it('confirms discarding edits before changing sections from the sidebar', async () => {
  wrapper = mount(App);
  await flushPromises();
  await wrapper.get('.add_button').trigger('click');
  await chooseFolders(wrapper);
  await wrapper.get('footer button:nth-child(2)').trigger('click');
  await chooseSchedule(wrapper);
  await wrapper.get('footer button:nth-child(2)').trigger('click');
  await wrapper.get('.filter_add').trigger('click');

  const cloudButton = wrapper
    .findAll('nav button')
    .find((item) => item.text() === messages.en.sections.cloud)!;
  await cloudButton.trigger('click');

  expect(wrapper.get('.nas_confirm_dialog').attributes('open')).toBeDefined();
  expect(wrapper.get('.nas_confirm_dialog h2').text()).toBe(messages.en.confirmDiscardTitle);
  await wrapper.get('.nas_confirm_dialog button').trigger('click');
  expect(wrapper.find('.nas_confirm_dialog').exists()).toBe(false);
  expect(wrapper.find('.rule').exists()).toBe(true);
  expect(wrapper.get('.rule h2').text()).toBe(messages.en.modalTitles.scheduled);

  await cloudButton.trigger('click');
  await wrapper.get('.nas_confirm_dialog button.app_button--primary').trigger('click');
  expect(wrapper.find('.nas_confirm_dialog').exists()).toBe(false);
  expect(wrapper.find('.rule').exists()).toBe(false);
  expect(wrapper.get('h1').text()).toBe(messages.en.sections.cloud);
});

it('confirms discarding edits when closing the rule wizard with its close button', async () => {
  wrapper = mount(App);
  await flushPromises();
  await wrapper.get('.add_button').trigger('click');
  await chooseFolders(wrapper);
  await wrapper.get('footer button:nth-child(2)').trigger('click');
  await chooseSchedule(wrapper);
  await wrapper.get('footer button:nth-child(2)').trigger('click');
  await wrapper.get('.filter_add').trigger('click');
  await wrapper.get('.close_button').trigger('click');

  expect(wrapper.get('.nas_confirm_dialog').attributes('open')).toBeDefined();
  await wrapper.get('.nas_confirm_dialog button.app_button--primary').trigger('click');
  expect(wrapper.find('.nas_confirm_dialog').exists()).toBe(false);
  expect(wrapper.find('.rule').exists()).toBe(false);
  expect(wrapper.get('h1').text()).toBe(messages.en.sections.scheduled);
});

it.each<SectionCode>(['scheduled', 'cloud', 'rtl'])(
  'opens and closes the %s configuration',
  async (section) => {
    wrapper = mount(App);
    await flushPromises();
    const navItem = wrapper
      .findAll('nav button')
      .find((item) => item.text() === messages.en.sections[section])!;
    await navItem.trigger('click');
    expect(navItem.attributes('aria-pressed')).toBe('true');
    expect(wrapper.get('h1').text()).toBe(messages.en.sections[section]);
    const add = wrapper.findAll('button').find((item) => item.text() === messages.en.add)!;
    await add.trigger('click');
    await vi.dynamicImportSettled();
    await flushPromises();
    expect(wrapper.get('.rule h2').text()).toBe(messages.en.modalTitles[section]);
    await wrapper.get(`[aria-label="${messages.en.close}"]`).trigger('click');
    expect(wrapper.find('.nas_confirm_dialog').exists()).toBe(false);
    expect(wrapper.find('.rule').exists()).toBe(false);
  },
);

it('points the Cloud page to the disk manager while no network disk is configured', async () => {
  vi.mocked(requestNetworkDisks).mockResolvedValue({ disks: [] });
  wrapper = mount(App);
  await flushPromises();
  expect(requestNetworkDisks).not.toHaveBeenCalled();
  const section = (name: SectionCode) =>
    wrapper!.findAll('nav button').find((item) => item.text() === messages.en.sections[name])!;

  await section('cloud').trigger('click');
  await flushPromises();
  expect(wrapper.get('[data-testid="network_disk_notice"]').text()).toContain(
    messages.en.networkDisks.noneTitle,
  );
  // No cloud rule without a network disk: the add button waits for one.
  expect(wrapper.find('[data-testid="rule_add"]').exists()).toBe(false);

  await section('scheduled').trigger('click');
  expect(wrapper.find('[data-testid="network_disk_notice"]').exists()).toBe(false);
  expect(wrapper.find('[data-testid="rule_add"]').exists()).toBe(true);

  vi.mocked(requestNetworkDisks).mockResolvedValue({ disks: [networkShare] });
  await section('cloud').trigger('click');
  await flushPromises();
  expect(wrapper.find('[data-testid="network_disk_notice"]').exists()).toBe(false);
  expect(wrapper.find('[data-testid="rule_add"]').exists()).toBe(true);
});

it('says when the network disks cannot be read, and reads them again on request', async () => {
  vi.mocked(requestNetworkDisks).mockResolvedValue({ disks: [], error: 'Launcher unavailable' });
  wrapper = mount(App);
  await flushPromises();
  const cloud = wrapper
    .findAll('nav button')
    .find((item) => item.text() === messages.en.sections.cloud)!;
  await cloud.trigger('click');
  await flushPromises();
  expect(wrapper.find('[data-testid="network_disk_notice"]').exists()).toBe(false);
  expect(wrapper.find('[data-testid="rule_add"]').exists()).toBe(false);
  expect(wrapper.get('[role="alert"]').text()).toContain(messages.en.networkDisks.loadError);

  vi.mocked(requestNetworkDisks).mockResolvedValue({ disks: [networkShare] });
  await wrapper.get('[data-testid="network_disks_retry"]').trigger('click');
  await flushPromises();
  expect(wrapper.find('[data-testid="network_disks_retry"]').exists()).toBe(false);
  expect(wrapper.find('[data-testid="rule_add"]').exists()).toBe(true);
});

/** A scheduled rule as the backend lists it. */
function listed(changes: Partial<ListedRule> = {}): ListedRule {
  return {
    id: 'photos',
    type: 'scheduled',
    name: 'Photos',
    enabled: true,
    createdAt: '2026-10-07T20:00:00.000Z',
    folders: { origin: '/Photos', destination: '/Backup/photos', cloud: null },
    sameDestination: false,
    syncMode: 'mirror',
    schedule: { frequency: 'daily', time: '02:00', weekday: null, day: null, month: null },
    filters: [],
    operations: [],
    operationConfigurations: { batchRename: null, archive: null, encrypt: null },
    duplicateResolution: 'overwrite',
    verifyArchivedFiles: false,
    lastRun: { at: Date.UTC(2026, 9, 10, 16, 30), status: 'failed', message: 'disk full' },
    nextRunAt: Date.UTC(2026, 9, 11, 0, 0),
    running: false,
    ...changes,
  };
}

it('lists the rules with how they last ran and when they run next', async () => {
  vi.mocked(fetchBackupRules).mockResolvedValue([
    listed(),
    listed({ id: 'off', name: 'Old', enabled: false, lastRun: null }),
  ]);
  wrapper = mount(App);
  await flushPromises();

  expect(wrapper.findAll('th').map((cell) => cell.text())).toEqual([
    'Name',
    'Schedule',
    'Enabled',
    'Last run',
    'Next run',
    'Created',
    'Actions',
  ]);
  const rows = wrapper.findAll('tbody tr');
  const cells = rows[0]!.findAll('td').map((cell) => cell.text());
  expect(cells[0]).toBe('Photos');
  expect(cells[1]).toBe('Daily · 02:00');
  expect(rows[0]!.findAll('td')[1]!.get('span').attributes('title')).toBe('Every day at 02:00');
  expect(cells[4]).toMatch(/10\/11\/26/);
  expect(rows[1]!.findAll('td')[4]!.text()).toBe(messages.en.ruleOff);
  expect(wrapper.get('[data-testid="rule_enabled_photos"]').attributes('aria-checked')).toBe(
    'true',
  );
  expect(wrapper.get('[data-testid="rule_enabled_off"]').attributes('aria-checked')).toBe('false');

  const status = wrapper.get('[data-testid="run_status_failed"]');
  expect(status.find('[role="tooltip"]').exists()).toBe(false);
  await status.trigger('mouseenter');
  const tooltip = status.get('[role="tooltip"]');
  expect(tooltip.text()).toContain(messages.en.runStatus.failed);
  expect(tooltip.text()).toContain('disk full');
  expect(tooltip.text()).toMatch(/Last run .*2026|Last run .*26/);
  expect(tooltip.attributes('style')).toContain('top');
  await status.trigger('mouseleave');
  expect(status.find('[role="tooltip"]').exists()).toBe(false);
  expect(wrapper.find('[data-testid="run_status_never"]').attributes('aria-label')).toBe(
    messages.en.never,
  );
});

it('runs, stops, switches and removes a rule from its row', async () => {
  vi.mocked(fetchBackupRules).mockResolvedValue([listed()]);
  wrapper = mount(App);
  await flushPromises();

  await wrapper.get('[data-testid="rule_run_photos"]').trigger('click');
  await flushPromises();
  expect(runBackupRule).toHaveBeenCalledWith('photos');

  vi.mocked(fetchBackupRules).mockResolvedValue([listed({ running: true })]);
  await wrapper.get('[data-testid="rule_enabled_photos"]').setValue(false);
  await flushPromises();
  expect(switchBackupRule).toHaveBeenCalledWith('photos', false);
  expect(wrapper.find('[data-testid="run_status_running"]').exists()).toBe(true);
  expect(wrapper.get('[data-testid="rule_edit_photos"]').attributes('disabled')).toBeDefined();
  expect(wrapper.get('[data-testid="rule_run_photos"]').attributes('aria-label')).toBe(
    'Stop Photos',
  );
  await wrapper.get('[data-testid="rule_run_photos"]').trigger('click');
  await flushPromises();
  expect(stopBackupRule).toHaveBeenCalledWith('photos');

  vi.mocked(fetchBackupRules).mockResolvedValue([listed()]);
  await wrapper.get('[data-testid="rule_run_photos"]').trigger('click');
  await flushPromises();
  await wrapper.get('[data-testid="rule_delete_photos"]').trigger('click');
  expect(wrapper.get('.nas_confirm_dialog h2').text()).toBe(messages.en.deleteTitle);
  expect(wrapper.get('.nas_confirm_dialog').text()).toContain('The rule Photos will be deleted');
  await wrapper.get('.nas_confirm_dialog button').trigger('click');
  expect(deleteBackupRule).not.toHaveBeenCalled();
  await wrapper.get('[data-testid="rule_delete_photos"]').trigger('click');
  await wrapper.get('.nas_confirm_dialog button.app_button--primary').trigger('click');
  await flushPromises();
  expect(deleteBackupRule).toHaveBeenCalledWith('photos');
});

it('says why an action on a rule failed', async () => {
  vi.mocked(fetchBackupRules).mockResolvedValue([listed()]);
  vi.mocked(runBackupRule).mockRejectedValueOnce(new Error('The rule is running already.'));
  wrapper = mount(App);
  await flushPromises();
  await wrapper.get('[data-testid="rule_run_photos"]').trigger('click');
  await flushPromises();
  expect(wrapper.get('.action_error').text()).toBe('The rule is running already.');
});

it('edits a rule in place', async () => {
  vi.mocked(fetchBackupRules).mockResolvedValue([listed()]);
  wrapper = mount(App);
  await flushPromises();
  await wrapper.get('[data-testid="rule_edit_photos"]').trigger('click');
  await flushPromises();

  expect(wrapper.get('.rule h2').text()).toBe(messages.en.editRule);
  const name = wrapper.get('[data-testid="rule_name"]');
  expect((name.element as HTMLInputElement).value).toBe('Photos');
  await name.setValue('Holiday photos');
  const next = () => wrapper!.findAll('footer button').find((button) => button.text() === 'Next')!;
  for (let step = 0; step < 4; step += 1) {
    await next().trigger('click');
    await flushPromises();
  }
  await wrapper
    .findAll('footer button')
    .find((button) => button.text() === 'Save')!
    .trigger('click');
  await flushPromises();

  expect(saveBackupRule).toHaveBeenCalledWith(
    expect.objectContaining({ id: 'photos', name: 'Holiday photos', enabled: true }),
    true,
  );
  expect(wrapper.find('.rule').exists()).toBe(false);
});

it('reads the list again while a rule runs and the page is in view', async () => {
  vi.useFakeTimers();
  vi.mocked(fetchBackupRules).mockResolvedValue([listed({ running: true })]);
  wrapper = mount(App);
  await flushPromises();
  const reads = vi.mocked(fetchBackupRules).mock.calls.length;

  await vi.advanceTimersByTimeAsync(3000);
  expect(vi.mocked(fetchBackupRules).mock.calls).toHaveLength(reads + 1);

  vi.mocked(fetchBackupRules).mockResolvedValue([listed()]);
  await vi.advanceTimersByTimeAsync(3000);
  const settled = vi.mocked(fetchBackupRules).mock.calls.length;
  await vi.advanceTimersByTimeAsync(9000);
  expect(vi.mocked(fetchBackupRules).mock.calls).toHaveLength(settled);
  vi.useRealTimers();
});
