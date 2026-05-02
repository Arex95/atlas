export const PROJECT_COLORS = [
  '#3b82f6',
  '#10b981',
  '#f59e0b',
  '#ef4444',
  '#8b5cf6',
  '#ec4899',
  '#06b6d4',
] as const;

export type ProjectColor = (typeof PROJECT_COLORS)[number];

export const DEFAULT_PROJECT_COLOR: ProjectColor = PROJECT_COLORS[0];
