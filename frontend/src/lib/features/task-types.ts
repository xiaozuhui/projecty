import type { TaskType } from '$lib/api/types';

export const taskTypeOptions: { value: TaskType; label: string }[] = [
  { value: 'feature', label: '功能' },
  { value: 'bug', label: '缺陷' },
  { value: 'design', label: '设计' },
  { value: 'revert', label: '回退' },
  { value: 'improvement', label: '优化' },
  { value: 'refactor', label: '重构' },
  { value: 'docs', label: '文档' },
  { value: 'chore', label: '维护' }
];

export function taskTypeLabel(value: string) {
  return taskTypeOptions.find((item) => item.value === value)?.label ?? value;
}
