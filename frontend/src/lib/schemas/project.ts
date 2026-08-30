export type SystemRole = 'super_admin' | 'user';
export type ProjectRole = 'manager' | 'member' | 'viewer';
export type EffectiveProjectRole = 'super_admin' | ProjectRole | 'none';

export type ProjectSummary = {
  id: string;
  projectKey: string;
  name: string;
  primaryDepartmentId?: string;
  archivedAt?: string;
};

export type TaskSummary = {
  id: string;
  taskKey: string;
  projectId: string;
  parentTaskId?: string;
  title: string;
  statusName: string;
  assigneeId?: string;
  updatedAt: string;
};
