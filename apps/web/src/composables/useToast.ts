import { ref } from 'vue';

export type ToastKind = 'info' | 'success' | 'warning' | 'error' | 'reminder';

export interface Toast {
  id: string;
  message: string;
  title?: string;
  kind: ToastKind;
}

const toasts = ref<Toast[]>([]);

let _seq = 0;

export function useToast() {
  function show(message: string, kind: ToastKind = 'info', title?: string, duration = 4000) {
    const id = `toast-${++_seq}`;
    toasts.value.push({ id, message, kind, title });
    setTimeout(() => dismiss(id), duration);
  }

  function dismiss(id: string) {
    toasts.value = toasts.value.filter((t) => t.id !== id);
  }

  return { toasts, show, dismiss };
}
