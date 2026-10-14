import { afterEach, expect, it } from 'vitest';
import { mount } from '@vue/test-utils';
import { nextTick } from 'vue';
import OriginFiltering from '@/components/rules/steps/OriginFilteringStep.vue';
import trashIcon from '@/assets/icons/trash.svg?url';
import { setLocale } from '@/i18n';
import type { OriginFilterRule } from '@/types/backup';

afterEach(() => setLocale('it'));

function mountFilters(rules: OriginFilterRule[]) {
  const wrapper = mount(OriginFiltering, {
    props: {
      title: 'Filters',
      rules,
      'onUpdate:rules': (value: OriginFilterRule[]) => wrapper.setProps({ rules: value }),
    },
  });
  return wrapper;
}

function lastRules(wrapper: ReturnType<typeof mountFilters>) {
  return wrapper.emitted<[OriginFilterRule[]]>('update:rules')?.at(-1)?.[0];
}

it('translates filter labels when the launcher language changes and keeps rule updates', async () => {
  setLocale('en');
  const rules: OriginFilterRule[] = [
    {
      id: 1,
      action: 'include',
      fileType: '*',
      status: 'updated',
      operator: 'before',
      value: 'today',
    },
  ];
  const wrapper = mount(OriginFiltering, { props: { title: 'Filters', rules } });
  expect(wrapper.text()).toContain('Action');
  expect(wrapper.text()).toContain('Include');
  expect(wrapper.get('[aria-label="Remove filter 1"] .app_icon').attributes('src')).toBe(trashIcon);
  expect(wrapper.get('[aria-label="Remove filter 1"] img').attributes('aria-hidden')).toBe('true');
  setLocale('it');
  await nextTick();
  expect(wrapper.text()).toContain('Azione');
  expect(wrapper.text()).toContain('Escludi');
  expect(wrapper.get('[aria-label="Rimuovi filtro 1"] .app_icon').attributes('src')).toBe(
    trashIcon,
  );
  await wrapper.findAll('button').at(-1)!.trigger('click');
  expect(wrapper.emitted('update:rules')?.[0]?.[0]).toHaveLength(2);
  wrapper.unmount();
});

it('adds rules with the next free id and removes them', async () => {
  setLocale('en');
  const wrapper = mountFilters([]);
  const add = wrapper.findAll('button').find((item) => item.text() === 'Add filter')!;
  await add.trigger('click');
  await add.trigger('click');
  expect(lastRules(wrapper)).toEqual([
    {
      id: 1,
      action: 'include',
      fileType: '*',
      status: 'updated',
      operator: 'before',
      value: 'today',
    },
    {
      id: 2,
      action: 'include',
      fileType: '*',
      status: 'updated',
      operator: 'before',
      value: 'today',
    },
  ]);
  await wrapper.get('[aria-label="Remove filter 1"]').trigger('click');
  expect(lastRules(wrapper)?.map((rule) => rule.id)).toEqual([2]);
  await add.trigger('click');
  expect(lastRules(wrapper)?.map((rule) => rule.id)).toEqual([2, 3]);
});

it('takes extensions, the action and the date filters, and says which extensions are not', async () => {
  setLocale('en');
  const wrapper = mountFilters([
    {
      id: 4,
      action: 'include',
      fileType: '*',
      status: 'updated',
      operator: 'before',
      value: 'today',
    },
  ]);
  await wrapper.get('[aria-label="File type, filter 4"]').setValue('.pdf, .docx');
  await wrapper.get('[aria-label="Action, filter 4"]').setValue('exclude');
  await wrapper.get('[aria-label="Filter, filter 4"]').setValue('after');
  await wrapper.get('[aria-label="Filter value, filter 4"]').setValue('lastWeek');
  expect(lastRules(wrapper)).toEqual([
    {
      id: 4,
      action: 'exclude',
      fileType: '.pdf, .docx',
      status: 'updated',
      operator: 'after',
      value: 'lastWeek',
    },
  ]);
  const files = wrapper.get('[aria-label="File type, filter 4"]');
  expect(files.attributes('aria-invalid')).toBe('false');
  await files.setValue('pdf');
  expect(files.attributes('aria-invalid')).toBe('true');
  expect(files.attributes('title')).toBe('Allowed values * or extensions like .jpg, .png');
  const updates = wrapper.emitted('update:rules')?.length;
  await wrapper.get('[aria-label="Action, filter 4"]').setValue('keep');
  await wrapper.get('[aria-label="Status, filter 4"]').setValue('owner');
  await wrapper.get('[aria-label="Filter, filter 4"]').setValue('equal');
  expect(wrapper.emitted('update:rules')).toHaveLength(updates!);
});

it('resets operator and value when the status changes', async () => {
  setLocale('en');
  const wrapper = mountFilters([
    {
      id: 1,
      action: 'include',
      fileType: '*',
      status: 'updated',
      operator: 'after',
      value: 'lastMonth',
    },
  ]);
  const status = () => wrapper.get('[aria-label="Status, filter 1"]');

  await status().setValue('size');
  expect(lastRules(wrapper)?.[0]).toMatchObject({ status: 'size', operator: 'greater', value: '' });
  const operator = wrapper.get('[aria-label="Filter, filter 1"]');
  expect(operator.findAll('option').map((option) => option.text())).toEqual([
    'greater than',
    'less than',
  ]);
  await operator.setValue('less');
  await wrapper.get('[aria-label="Size in MB, filter 1"]').setValue('500');
  expect(lastRules(wrapper)?.[0]).toMatchObject({ operator: 'less', value: '500' });

  await status().setValue('name');
  expect(lastRules(wrapper)?.[0]).toMatchObject({ status: 'name', operator: null, value: '' });
  expect(wrapper.find('[aria-label="Filter, filter 1"]').exists()).toBe(false);
  await wrapper.get('[aria-label="File name, filter 1"]').setValue('report');
  expect(lastRules(wrapper)?.[0]).toMatchObject({ value: 'report' });

  await status().setValue('created');
  expect(lastRules(wrapper)?.[0]).toMatchObject({
    status: 'created',
    operator: 'before',
    value: 'today',
  });
  expect(wrapper.get('[aria-label="Filter, filter 1"]').findAll('option')).toHaveLength(4);
});
