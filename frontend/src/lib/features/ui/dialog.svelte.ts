// promise 式对话框:原生 confirm/prompt/alert 的替代。
// 同一时刻只允许一个对话框,已有对话框打开时后来的调用直接按取消处理。
export type DialogOptions = {
  title: string;
  message?: string;
  confirmLabel?: string;
  danger?: boolean;
};

export type PromptOptions = DialogOptions & {
  label?: string;
  placeholder?: string;
  initial?: string;
};

type DialogState =
  | { kind: 'closed' }
  | ({ kind: 'confirm'; resolve: (value: boolean) => void } & DialogOptions)
  | ({ kind: 'prompt'; resolve: (value: string | null) => void } & PromptOptions)
  | ({ kind: 'alert'; resolve: () => void } & DialogOptions);

export const dialog = $state<{ current: DialogState }>({ current: { kind: 'closed' } });

export function confirmDialog(options: DialogOptions): Promise<boolean> {
  if (dialog.current.kind !== 'closed') return Promise.resolve(false);
  return new Promise((resolve) => {
    dialog.current = { kind: 'confirm', ...options, resolve };
  });
}

export function promptDialog(options: PromptOptions): Promise<string | null> {
  if (dialog.current.kind !== 'closed') return Promise.resolve(null);
  return new Promise((resolve) => {
    dialog.current = { kind: 'prompt', ...options, resolve };
  });
}

export function alertDialog(options: DialogOptions): Promise<void> {
  if (dialog.current.kind !== 'closed') return Promise.resolve();
  return new Promise((resolve) => {
    dialog.current = { kind: 'alert', ...options, resolve };
  });
}
