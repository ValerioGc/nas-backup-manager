import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import App from '@/App.vue';
import CommonRuleView from '@/components/rules/CommonRuleView.vue';
import arrowLeft from '@/assets/icons/arrow-left.svg?url';
import arrowRight from '@/assets/icons/arrow-right.svg?url';
import { messages, setLocale } from '@/i18n';
import { requestPowerSchedule } from '@/services/nasShell';
import type { BackupRule, SectionCode } from '@/types/backup';
import { answerPickers, chooseFolders } from '../../helpers/folders';
import { answerPowerSchedule, chooseSchedule } from '../../helpers/schedule';

// The backend, as far as the wizard can tell: what is saved is listed, never run yet.
const saved: BackupRule[] = [];
vi.mock('@/services/backupRules', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/services/backupRules')>()),
  fetchBackupRules: vi.fn(async () =>
    saved.map((rule) => ({ ...rule, lastRun: null, nextRunAt: null, running: false })),
  ),
  saveBackupRule: vi.fn(async (rule: BackupRule) => {
    saved.push(rule);
    return rule;
  }),
}));

vi.mock('@/services/nasShell', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/services/nasShell')>()),
  pickNasFolder: vi.fn(),
  requestNetworkDisks: vi.fn(),
  requestPowerSchedule: vi.fn(),
}));

beforeEach(() => {
  vi.clearAllMocks();
  saved.length = 0;
  answerPickers();
  answerPowerSchedule();
});
afterEach(() => {
  setLocale('it');
  vi.unstubAllGlobals();
});

it.each<[SectionCode, string, number]>([
  ['scheduled', messages.en.stepLabels.folders, 5],
  ['cloud', messages.en.stepLabels.cloudFolder, 5],
  ['rtl', messages.en.stepLabels.folders, 4],
])('shows the first step for the %s section', (section, firstStep, steps) => {
  setLocale('en');
  const wrapper = mount(CommonRuleView, { props: { section } });

  expect(wrapper.get('h2').text()).toBe(messages.en.modalTitles[section]);
  expect(wrapper.get('h3').text()).toBe(firstStep);
  expect(wrapper.get('progress').attributes('aria-valuetext')).toBe(`1 / ${steps}`);
  expect(wrapper.get('footer button:first-child img').attributes('src')).toBe(arrowLeft);
  expect(wrapper.get('footer button:last-child img').attributes('src')).toBe(arrowRight);
});

it('keeps filters across steps and saves them with the selected section', async () => {
  setLocale('en');
  const wrapper = mount(CommonRuleView, { props: { section: 'cloud' } });
  const footerButton = (label: string) =>
    wrapper.findAll('footer button').find((button) => button.text() === label)!;

  await chooseFolders(wrapper);
  await footerButton('Next').trigger('click');
  await chooseSchedule(wrapper);
  await footerButton('Next').trigger('click');
  await wrapper.findAll('section button').at(-1)!.trigger('click');
  await footerButton('Next').trigger('click');
  await footerButton('Back').trigger('click');
  expect(wrapper.findAll('tbody tr')).toHaveLength(1);
  await footerButton('Next').trigger('click');
  await footerButton('Next').trigger('click');
  expect(wrapper.get('h3').text()).toBe(messages.en.stepLabels.destination);
  await footerButton('Save').trigger('click');

  expect(wrapper.emitted('save')?.[0]?.[0]).toMatchObject({
    type: 'cloud',
    id: expect.any(String),
    folders: { origin: null, destination: null, cloud: '/mnt/office' },
    schedule: { frequency: 'daily', time: '02:00', weekday: null, day: null, month: null },
    name: 'Rule',
    filters: [
      {
        id: 1,
        action: 'include',
        fileType: '*',
        status: 'updated',
        operator: 'before',
        value: 'today',
      },
    ],
    createdAt: expect.any(String),
  });
});

it('goes past the folders only once they are chosen and saves them with the rule', async () => {
  vi.stubGlobal('crypto', { getRandomValues: crypto.getRandomValues.bind(crypto) });
  setLocale('en');
  const wrapper = mount(CommonRuleView, { props: { section: 'scheduled' } });
  const footerButton = (label: string) =>
    wrapper.findAll('footer button').find((button) => button.text() === label)!;

  expect(footerButton('Next').attributes('disabled')).toBeDefined();
  await footerButton('Next').trigger('click');
  expect(wrapper.get('h3').text()).toBe(messages.en.stepLabels.folders);

  await chooseFolders(wrapper);
  expect(wrapper.findAll('.folder_path').map((path) => path.text())).toEqual([
    '/Folder 1',
    '/Folder 2',
  ]);
  expect(wrapper.emitted('dirty-change')?.at(-1)).toEqual([true]);
  await footerButton('Next').trigger('click');
  await chooseSchedule(wrapper);
  await footerButton('Next').trigger('click');
  await footerButton('Next').trigger('click');
  await footerButton('Next').trigger('click');
  await footerButton('Save').trigger('click');

  expect(wrapper.emitted('save')?.[0]?.[0]).toMatchObject({
    type: 'scheduled',
    id: expect.stringMatching(/^[\da-f]{8}-[\da-f]{4}-4[\da-f]{3}-[89ab][\da-f]{3}-[\da-f]{12}$/),
    folders: { origin: '/Folder 1', destination: '/Folder 2', cloud: null },
  });
});

it('resets the wizard when the selected section changes', async () => {
  setLocale('en');
  const wrapper = mount(CommonRuleView, { props: { section: 'scheduled' } });
  const next = () => wrapper.findAll('footer button').find((button) => button.text() === 'Next')!;

  await chooseFolders(wrapper);
  await next().trigger('click');
  await chooseSchedule(wrapper);
  await next().trigger('click');
  await wrapper.findAll('section button').at(-1)!.trigger('click');
  await wrapper.setProps({ section: 'rtl' });

  expect(wrapper.get('h3').text()).toBe(messages.en.stepLabels.folders);
  expect(wrapper.get('progress').attributes('aria-valuetext')).toBe('1 / 4');
  expect(wrapper.findAll('tbody tr')).toHaveLength(0);
});

it('keeps destination choices while navigating and marks them dirty', async () => {
  setLocale('en');
  const wrapper = mount(CommonRuleView, { props: { section: 'scheduled' } });
  const footerButton = (label: string) =>
    wrapper.findAll('footer button').find((button) => button.text() === label)!;

  await chooseFolders(wrapper);
  await footerButton('Next').trigger('click');
  await chooseSchedule(wrapper);
  await footerButton('Next').trigger('click');
  await footerButton('Next').trigger('click');
  await footerButton('Next').trigger('click');
  await wrapper.get('select').setValue('copy');
  await wrapper.get('input[type="checkbox"]').setValue(true);

  expect(wrapper.emitted('dirty-change')?.some(([isDirty]) => isDirty === true)).toBe(true);

  await footerButton('Back').trigger('click');
  await footerButton('Next').trigger('click');
  expect(wrapper.get('select').element.value).toBe('copy');
  expect((wrapper.get('input[type="checkbox"]').element as HTMLInputElement).checked).toBe(true);

  await wrapper.setProps({ section: 'rtl' });
  expect(wrapper.get('h3').text()).toBe(messages.en.stepLabels.folders);
  expect(wrapper.emitted('dirty-change')?.at(-1)).toEqual([false]);
});

it('keeps selected file operations while navigating and resets them on section change', async () => {
  setLocale('en');
  const wrapper = mount(CommonRuleView, { props: { section: 'scheduled' } });
  const footerButton = (label: string) =>
    wrapper.findAll('footer button').find((button) => button.text() === label)!;

  await chooseFolders(wrapper);
  await footerButton('Next').trigger('click');
  await chooseSchedule(wrapper);
  await footerButton('Next').trigger('click');
  await footerButton('Next').trigger('click');
  const renameOperation = wrapper.findAll('.operation_card')[0];
  await renameOperation.trigger('click');
  await wrapper.get('#rename-filename').setValue('report.txt');
  await wrapper.get('#rename-prefix').setValue('2026');
  await wrapper.get('form').trigger('submit');

  expect(renameOperation.attributes('aria-pressed')).toBe('true');
  expect(wrapper.emitted('dirty-change')?.some(([isDirty]) => isDirty === true)).toBe(true);

  await footerButton('Next').trigger('click');
  await footerButton('Back').trigger('click');
  expect(wrapper.findAll('.operation_card')[0].attributes('aria-pressed')).toBe('true');

  await wrapper.setProps({ section: 'rtl' });
  expect(wrapper.find('.operation_card').exists()).toBe(false);
  expect(wrapper.emitted('dirty-change')?.at(-1)).toEqual([false]);
});

it('submits modal options with the rule only at the final step', async () => {
  setLocale('en');
  const wrapper = mount(CommonRuleView, { props: { section: 'scheduled' } });
  const footerButton = (label: string) =>
    wrapper.findAll('footer button').find((button) => button.text() === label)!;

  await chooseFolders(wrapper);
  await footerButton('Next').trigger('click');
  await chooseSchedule(wrapper);
  await footerButton('Next').trigger('click');
  await footerButton('Next').trigger('click');
  await wrapper.findAll('.operation_card')[1]!.trigger('click');
  await wrapper.get('#archive-name').setValue('daily');
  await wrapper.get('#operation-password').setValue('secret');
  await wrapper.get('#operation-password-confirmation').setValue('secret');
  await wrapper.get('[data-testid="verify_archive"]').setValue(true);
  await wrapper.get('form').trigger('submit');

  expect(wrapper.emitted('save')).toBeUndefined();
  await footerButton('Next').trigger('click');
  expect(wrapper.emitted('save')).toBeUndefined();
  await footerButton('Save').trigger('click');

  expect(wrapper.emitted('save')?.[0]?.[0]).toMatchObject({
    type: 'scheduled',
    operations: ['archive'],
    operationConfigurations: {
      archive: {
        archiveName: 'daily',
        password: 'secret',
        verifyArchive: true,
      },
    },
  });
});

it('saves the rule under the section where it was created', async () => {
  setLocale('en');
  const wrapper = mount(App);
  await flushPromises();
  const cloudSection = wrapper
    .findAll('nav button')
    .find((button) => button.text() === messages.en.sections.cloud)!;

  await cloudSection.trigger('click');
  await wrapper
    .findAll('button')
    .find((button) => button.text() === messages.en.add)!
    .trigger('click');

  const footerButton = (label: string) =>
    wrapper.findAll('footer button').find((button) => button.text() === label)!;
  await chooseFolders(wrapper);
  await footerButton('Next').trigger('click');
  await chooseSchedule(wrapper);
  await footerButton('Next').trigger('click');
  await wrapper.findAll('section button').at(-1)!.trigger('click');
  await footerButton('Next').trigger('click');
  await footerButton('Next').trigger('click');
  await footerButton('Save').trigger('click');

  await flushPromises();
  expect(wrapper.get('h1').text()).toBe(messages.en.sections.cloud);
  const cells = wrapper.findAll('tbody tr td').map((cell) => cell.text());
  expect(cells[0]).toBe('Rule');
  expect(cells[1]).toBe('Daily · 02:00');
  expect(cells[4]).toBe('-');
  expect(wrapper.find('[data-testid="run_status_never"]').exists()).toBe(true);
  expect(wrapper.find('[data-testid^="rule_run_"]').exists()).toBe(false);
});

it('goes past the schedule only at a time the NAS is on', async () => {
  setLocale('en');
  answerPowerSchedule({
    enabled: true,
    rules: [
      { day: 'everyday', time: '23:00', action: 'shutdown' },
      { day: 'everyday', time: '07:00', action: 'start' },
    ],
  });
  const wrapper = mount(CommonRuleView, { props: { section: 'scheduled' } });
  const footerButton = (label: string) =>
    wrapper.findAll('footer button').find((button) => button.text() === label)!;

  await chooseFolders(wrapper);
  await footerButton('Next').trigger('click');
  await flushPromises();
  expect(wrapper.get('h3').text()).toBe(messages.en.stepLabels.schedule);
  expect(footerButton('Next').attributes('disabled')).toBeDefined();

  await chooseSchedule(wrapper, '23:30');
  expect(wrapper.find('[data-testid="schedule_off"]').exists()).toBe(true);
  expect(footerButton('Next').attributes('disabled')).toBeDefined();
  await footerButton('Next').trigger('click');
  expect(wrapper.get('h3').text()).toBe(messages.en.stepLabels.schedule);

  await chooseSchedule(wrapper, '12:00');
  expect(footerButton('Next').attributes('disabled')).toBeUndefined();
  await footerButton('Next').trigger('click');
  expect(wrapper.get('h3').text()).toBe(messages.en.stepLabels.filtering);
});

it('saves a real-time rule without a schedule, with its destination options', async () => {
  setLocale('en');
  const wrapper = mount(CommonRuleView, { props: { section: 'rtl' } });
  const footerButton = (label: string) =>
    wrapper.findAll('footer button').find((button) => button.text() === label)!;

  await chooseFolders(wrapper);
  await footerButton('Next').trigger('click');
  expect(wrapper.get('h3').text()).toBe(messages.en.stepLabels.filtering);
  await footerButton('Next').trigger('click');
  await footerButton('Next').trigger('click');
  await wrapper.get('select').setValue('ignore');
  await wrapper.get('input[type="checkbox"]').setValue(true);
  await footerButton('Save').trigger('click');

  expect(requestPowerSchedule).not.toHaveBeenCalled();
  expect(wrapper.emitted('save')?.[0]?.[0]).toMatchObject({
    type: 'rtl',
    schedule: null,
    duplicateResolution: 'ignore',
    verifyArchivedFiles: true,
  });
});

it('edits a saved rule: its choices filled in, its id kept, encryption left out', async () => {
  setLocale('en');
  const initial: BackupRule = {
    id: 'kept',
    type: 'rtl',
    name: 'Photos',
    enabled: false,
    createdAt: '2026-01-01T00:00:00.000Z',
    folders: { origin: '/Photos', destination: '/Backup/photos', cloud: null },
    sameDestination: false,
    syncMode: 'copy',
    schedule: null,
    filters: [],
    operations: ['archive', 'encrypt'],
    operationConfigurations: {
      batchRename: null,
      archive: { archiveName: 'daily', password: '', encryptFileInfo: true, verifyArchive: false },
      encrypt: { mode: 'encrypt', output: 'single', password: '' },
    },
    duplicateResolution: 'ignore',
    verifyArchivedFiles: true,
  };
  const wrapper = mount(CommonRuleView, { props: { section: 'rtl', initial } });
  const footerButton = (label: string) =>
    wrapper.findAll('footer button').find((button) => button.text() === label)!;

  expect(wrapper.get('h2').text()).toBe(messages.en.editRule);
  expect(wrapper.emitted('dirty-change')?.at(-1)).toEqual([false]);
  await footerButton('Next').trigger('click');
  await footerButton('Next').trigger('click');
  await wrapper.findAll('.operation_card')[1]!.trigger('click');
  expect(wrapper.get('#operation-password').attributes('placeholder')).toBe(
    messages.en.operationModals.keepPassword,
  );
  await wrapper.get('form').trigger('submit');
  await footerButton('Next').trigger('click');
  expect(wrapper.get('[data-testid="sync_copy"]').element).toHaveProperty('checked', true);
  await footerButton('Save').trigger('click');

  expect(wrapper.emitted('save')?.[0]?.[0]).toMatchObject({
    id: 'kept',
    enabled: false,
    createdAt: '2026-01-01T00:00:00.000Z',
    operations: ['archive'],
    operationConfigurations: { encrypt: null, archive: { password: '' } },
    syncMode: 'copy',
  });
});

it('writes into the watched folder only with an operation, and never mirrors it', async () => {
  setLocale('en');
  const wrapper = mount(CommonRuleView, { props: { section: 'rtl' } });
  const footerButton = (label: string) =>
    wrapper.findAll('footer button').find((button) => button.text() === label)!;

  await wrapper.get('[data-testid="same_destination"]').setValue(true);
  await chooseFolders(wrapper);
  expect(wrapper.findAll('.folder_card')).toHaveLength(1);
  await footerButton('Next').trigger('click');
  await footerButton('Next').trigger('click');
  expect(wrapper.get('h3').text()).toBe(messages.en.stepLabels.operations);
  expect(footerButton('Next').attributes('disabled')).toBeDefined();
  await wrapper.findAll('.operation_card')[0]!.trigger('click');
  await wrapper.get('#rename-filename').setValue('report.txt');
  await wrapper.get('#rename-prefix').setValue('bak');
  await wrapper.get('form').trigger('submit');
  await footerButton('Next').trigger('click');
  expect(wrapper.find('[data-testid="sync_mirror"]').exists()).toBe(false);
  await footerButton('Save').trigger('click');

  expect(wrapper.emitted('save')?.[0]?.[0]).toMatchObject({
    sameDestination: true,
    syncMode: 'copy',
    folders: { origin: '/Folder 1', destination: null },
    operations: ['batchRename'],
  });
});

it('goes past the filters only when they are complete', async () => {
  setLocale('en');
  const wrapper = mount(CommonRuleView, { props: { section: 'rtl' } });
  const footerButton = (label: string) =>
    wrapper.findAll('footer button').find((button) => button.text() === label)!;
  expect(footerButton('Next').attributes('disabled')).toBeDefined();
  await chooseFolders(wrapper);
  await footerButton('Next').trigger('click');
  await wrapper.get('.filter_add').trigger('click');
  await wrapper.get('[aria-label="File type, filter 1"]').setValue('pdf');
  expect(footerButton('Next').attributes('disabled')).toBeDefined();
  await wrapper.get('[aria-label="File type, filter 1"]').setValue('.pdf');
  expect(footerButton('Next').attributes('disabled')).toBeUndefined();
});
