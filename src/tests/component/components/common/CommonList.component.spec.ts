import { expect, it } from 'vitest';
import { mount } from '@vue/test-utils';
import CommonList from '@/components/common/CommonList.vue';

const columns = [
  { key: 'name', label: 'Name' },
  { key: 'lastRun', label: 'Last run' },
];

it('renders one row per rule and a dash for the missing values', () => {
  const wrapper = mount(CommonList, {
    props: {
      title: 'Scheduled',
      columns,
      list: [
        { id: 'a', name: 'Documents', lastRun: '2026-10-01' },
        { id: 'b', name: 'Photos', lastRun: null },
      ],
      addLabel: 'Add',
      emptyIcon: '/scheduled.svg',
      emptyTitle: 'Nothing here',
      emptyDescription: 'Add a rule',
    },
  });
  expect(wrapper.get('h1').text()).toBe('Scheduled');
  expect(wrapper.findAll('th').map((cell) => cell.text())).toEqual(['Name', 'Last run']);
  expect(wrapper.findAll('tbody td').map((cell) => cell.text())).toEqual([
    'Documents',
    '2026-10-01',
    'Photos',
    '-',
  ]);
  expect(wrapper.text()).not.toContain('Nothing here');
  expect(wrapper.find('.empty_state').exists()).toBe(false);
});

it('shows an empty placeholder without duplicating the page add button', async () => {
  const wrapper = mount(CommonList, {
    props: {
      title: 'Cloud',
      columns,
      list: [],
      addLabel: 'Add',
      emptyIcon: '/cloud.svg',
      emptyTitle: 'Nothing here',
      emptyDescription: 'Add a rule',
    },
  });
  expect(wrapper.get('.empty_state h2').text()).toBe('Nothing here');
  expect(wrapper.get('.empty_state p').text()).toBe('Add a rule');
  expect(wrapper.get('.empty_icon').attributes('src')).toBe('/cloud.svg');
  expect(wrapper.findAll('button')).toHaveLength(1);
  await wrapper.get('.add_button').trigger('click');
  expect(wrapper.emitted('add')).toHaveLength(1);
});

it('lets the page draw a cell and the actions of each row', () => {
  const wrapper = mount(CommonList, {
    props: {
      title: 'Rules',
      columns,
      list: [{ id: 'a', name: 'Photos', lastRun: null }],
      addLabel: 'Add',
      emptyIcon: '/icon.svg',
      emptyTitle: 'None',
      emptyDescription: 'Add one',
      actionsLabel: 'Actions',
    },
    slots: {
      cell_lastRun: '<template #cell_lastRun="{ row }"><b>{{ row.id }} drawn</b></template>',
      actions: '<template #actions="{ row }"><button>Run {{ row.name }}</button></template>',
    },
  });
  expect(wrapper.findAll('th').at(-1)!.text()).toBe('Actions');
  expect(wrapper.get('b').text()).toBe('a drawn');
  expect(wrapper.get('.list_actions button').text()).toBe('Run Photos');
});
