// Categorical colors for document types: fixed slots, assigned by the bundle-wide type
// order (most documents first), so a filter never repaints a type. Types past the
// seventh share the neutral "Other" color.

const LIGHT = ["#2a78d6", "#eb6834", "#1baf7a", "#eda100", "#e87ba4", "#008300", "#4a3aa7"];
const DARK = ["#3987e5", "#d95926", "#199e70", "#c98500", "#d55181", "#008300", "#9085e9"];
const OTHER = { light: "#8f8e88", dark: "#77766f" };

export const isDark = () =>
  document.documentElement.dataset.theme === "dark" ||
  (document.documentElement.dataset.theme !== "light" &&
    matchMedia("(prefers-color-scheme: dark)").matches);

export interface TypePalette {
  color: (type: string | null) => string;
  legend: { type: string; color: string }[];
}

export function typePalette(types: Record<string, number>): TypePalette {
  const order = Object.entries(types)
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .map(([t]) => t);
  const dark = isDark();
  const ramp = dark ? DARK : LIGHT;
  const other = dark ? OTHER.dark : OTHER.light;
  const slots = new Map(order.slice(0, ramp.length).map((t, i) => [t, ramp[i]]));
  const legend = [...slots].map(([type, color]) => ({ type, color }));
  if (order.length > ramp.length || order.length === 0) legend.push({ type: "Other", color: other });
  return { color: (t) => (t && slots.get(t)) || other, legend };
}

export const cssVar = (name: string) =>
  getComputedStyle(document.documentElement).getPropertyValue(name).trim();
