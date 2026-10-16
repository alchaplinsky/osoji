import type { Crumb } from "../../lib/api";

type Props = { ancestors: Crumb[]; current: Crumb | null; onNavigate: (id: number) => void };

export default function Breadcrumbs({ ancestors, current, onNavigate }: Props) {
  if (!current) return null;
  return (
    <div className="crumbs">
      {ancestors.map((c) => (
        <span key={c.id} className="row">
          <button onClick={() => onNavigate(c.id)}>{c.name}</button>
          <span className="sep">›</span>
        </span>
      ))}
      <b>{current.name}</b>
    </div>
  );
}
