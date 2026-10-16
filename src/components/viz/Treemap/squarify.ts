export type Rect = { x: number; y: number; w: number; h: number };

export function squarify<T extends { value: number }>(items: T[], x: number, y: number, w: number, h: number): (T & Rect)[] {
  const out: (T & Rect)[] = [];
  const list = items.filter((it) => it.value > 0);
  const total = list.reduce((s, it) => s + it.value, 0);
  if (!total || w <= 0 || h <= 0) return out;

  const scale = (w * h) / total;
  const area = (it: T) => it.value * scale;
  const sumOf = (row: T[]) => row.reduce((s, it) => s + area(it), 0);
  let rx = x;
  let ry = y;
  let rw = w;
  let rh = h;

  const worst = (row: T[], side: number) => {
    const sum = sumOf(row);
    let max = 0;
    for (const it of row) {
      const a = area(it);
      max = Math.max(max, (side * side * a) / (sum * sum), (sum * sum) / (side * side * a));
    }
    return max;
  };

  const layout = (row: T[]) => {
    const sum = sumOf(row);
    if (rw >= rh) {
      const cw = sum / rh;
      let cy = ry;
      for (const it of row) {
        const ch = area(it) / cw;
        out.push({ ...it, x: rx, y: cy, w: cw, h: ch });
        cy += ch;
      }
      rx += cw;
      rw -= cw;
    } else {
      const ch = sum / rw;
      let cx = rx;
      for (const it of row) {
        const cw = area(it) / ch;
        out.push({ ...it, x: cx, y: ry, w: cw, h: ch });
        cx += cw;
      }
      ry += ch;
      rh -= ch;
    }
  };

  let row: T[] = [];
  for (const it of list) {
    const side = Math.min(rw, rh);
    if (row.length && worst([...row, it], side) > worst(row, side)) {
      layout(row);
      row = [it];
    } else {
      row.push(it);
    }
  }
  if (row.length) layout(row);
  return out;
}
