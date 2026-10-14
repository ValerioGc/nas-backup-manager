import { beforeEach, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import OriginDestinationStep from '@/components/rules/steps/OriginDestinationStep.vue';
import { messages, setLocale } from '@/i18n';
import { pickNasFolder, requestNetworkDisks } from '@/services/nasShell';
import type { RuleFolders, SectionCode } from '@/types/backup';
import { answerPickers } from '../../helpers/folders';
import { forgetVolumeNames } from '@/composables/useVolumeNames';

vi.mock('@/services/nasShell', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/services/nasShell')>()),
  pickNasFolder: vi.fn(),
  requestNetworkDisks: vi.fn(),
}));

const empty: RuleFolders = { origin: null, destination: null, cloud: null };

function mountStep(type: SectionCode, folders: RuleFolders = empty) {
  const wrapper = mount(OriginDestinationStep, {
    props: {
      title: 'Folders',
      type,
      folders,
      name: '',
      sameDestination: false,
      'onUpdate:folders': (value: RuleFolders) => wrapper.setProps({ folders: value }),
      'onUpdate:name': (value: string) => wrapper.setProps({ name: value }),
      'onUpdate:sameDestination': (value: boolean) => wrapper.setProps({ sameDestination: value }),
    },
  });
  return wrapper;
}

beforeEach(() => {
  setLocale('en');
  vi.mocked(pickNasFolder).mockReset();
  answerPickers();
});

it.each<[SectionCode, string[]]>([
  ['scheduled', [messages.en.folders.origin, messages.en.folders.destination]],
  ['rtl', [messages.en.folders.watched, messages.en.folders.destination]],
])('asks the %s backups for their folders', (type, labels) => {
  const wrapper = mountStep(type);

  expect(wrapper.get('h3').text()).toBe('Folders');
  expect(wrapper.findAll('.folder_card h4').map((label) => label.text())).toEqual(labels);
  expect(wrapper.findAll('.folder_empty').map((path) => path.text())).toEqual(
    labels.map(() => messages.en.folders.notSelected),
  );
  expect(wrapper.findAll('.folder_card button').map((button) => button.text())).toEqual(
    labels.map(() => messages.en.folders.select),
  );
});

it('picks a folder through the launcher, starting from the one already chosen', async () => {
  const wrapper = mountStep('scheduled', { ...empty, origin: '/Photos' });

  await wrapper.findAll('.folder_card button')[0]!.trigger('click');
  await flushPromises();
  expect(pickNasFolder).toHaveBeenCalledWith('/Photos');
  expect(wrapper.emitted('update:folders')?.at(-1)).toEqual([{ ...empty, origin: '/Folder 1' }]);
  expect(wrapper.findAll('.folder_card button')[0]!.text()).toBe(messages.en.folders.change);

  await wrapper.findAll('.folder_card button')[1]!.trigger('click');
  await flushPromises();
  expect(pickNasFolder).toHaveBeenLastCalledWith(undefined);
  expect(wrapper.findAll('.folder_path').map((path) => path.text())).toEqual([
    '/Folder 1',
    '/Folder 2',
  ]);
});

it('names the rule, and lets a real-time one write into the folder it watches', async () => {
  const wrapper = mountStep('rtl');
  await wrapper.get('[data-testid="rule_name"]').setValue('Photos');
  expect(wrapper.emitted('update:name')?.at(-1)).toEqual(['Photos']);

  await wrapper.get('[data-testid="same_destination"]').setValue(true);
  expect(wrapper.emitted('update:sameDestination')?.at(-1)).toEqual([true]);
  expect(wrapper.findAll('.folder_card h4').map((label) => label.text())).toEqual([
    messages.en.folders.watched,
  ]);
  expect(mountStep('scheduled').find('[data-testid="same_destination"]').exists()).toBe(false);
});

it('keeps the folder when the picker is cancelled and says when it cannot open', async () => {
  const wrapper = mountStep('scheduled', { ...empty, origin: '/Backup' });
  vi.mocked(pickNasFolder).mockResolvedValueOnce({ files: [], cancelled: true });

  await wrapper.get('.folder_card button').trigger('click');
  await flushPromises();
  expect(wrapper.emitted('update:folders')).toBeUndefined();
  expect(wrapper.find('[role="alert"]').exists()).toBe(false);

  vi.mocked(pickNasFolder).mockResolvedValueOnce({
    files: [],
    cancelled: false,
    error: 'Launcher unavailable',
  });
  await wrapper.get('.folder_card button').trigger('click');
  await flushPromises();
  expect(wrapper.get('[role="alert"]').text()).toBe(messages.en.folders.pickerError);
  expect(wrapper.get('.folder_path').text()).toBe('/Backup');
});

it('lets the cloud backups choose only among the network disks', async () => {
  vi.mocked(requestNetworkDisks).mockResolvedValue({
    disks: [
      { name: 'office', mount: '/mnt/office', source: '//nas/office', readOnly: false },
      { name: 'archive', mount: '/mnt/archive', source: null, readOnly: true },
    ],
  });
  const wrapper = mountStep('cloud');
  expect(wrapper.get('output').text()).toBe(messages.en.networkDisks.loading);
  await flushPromises();

  expect(wrapper.find('.folder_card button').exists()).toBe(false);
  const options = wrapper.findAll('input[name="network-disk"]');
  expect(options.map((option) => option.attributes('disabled') !== undefined)).toEqual([
    false,
    true,
  ]);
  expect(wrapper.text()).toContain('//nas/office');
  expect(wrapper.text()).toContain('/mnt/archive');
  expect(wrapper.text()).toContain(messages.en.networkDisks.readOnly);

  await options[0]!.setValue(true);
  expect(wrapper.emitted('update:folders')?.at(-1)).toEqual([{ ...empty, cloud: '/mnt/office' }]);
});

it('shows a folder by the name of its disk, not by its id', async () => {
  forgetVolumeNames();
  vi.stubGlobal(
    'fetch',
    vi.fn().mockResolvedValue(new Response(JSON.stringify([{ id: 'f00d', name: 'Dati' }]))),
  );
  const wrapper = mountStep('scheduled', { ...empty, origin: '/f00d/Photos/2026' });
  await flushPromises();

  const origin = wrapper.findAll('.folder_card')[0]!;
  expect(origin.get('.folder_disk').text()).toBe('Dati');
  expect(origin.get('.folder_path').text()).toBe('/Photos/2026');
  expect(origin.get('.folder_location').attributes('title')).toBe('/f00d/Photos/2026');
  expect(origin.get('button').text()).toBe(messages.en.folders.change);
  expect(origin.get('button').attributes('aria-label')).toBe(
    `${messages.en.folders.change}: ${messages.en.folders.origin}`,
  );
  expect(wrapper.find('.folder_arrow').exists()).toBe(true);
  vi.unstubAllGlobals();
  forgetVolumeNames();
});
