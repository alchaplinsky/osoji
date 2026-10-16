import { ask, message } from "@tauri-apps/plugin-dialog";
import { reveal, trashNodes, type NodeInfo, type ViewNode } from "../../lib/api";
import { humanBytes, humanCount } from "../../lib/format";

type Props = {
  hovered: ViewNode | null;
  hoveredParent: ViewNode | null;
  selected: NodeInfo | null;
  onTrashed: () => void;
};

export default function NodeDetails({ hovered, hoveredParent, selected, onTrashed }: Props) {
  const node = hovered ?? selected;
  if (!node) return <div className="note">Hover a slice to inspect it. Click a folder to drill in, click the center to go up.</div>;

  const share = hovered && hoveredParent && hoveredParent.size > 0 ? (hovered.size / hoveredParent.size) * 100 : null;

  async function doTrash() {
    if (!selected) return;
    const ok = await ask(
      `Move "${selected.name}" (${humanBytes(selected.size)}) to the Trash?\n\nSpace is reclaimed once you empty the Trash.`,
      { title: "Move to Trash", kind: "warning", okLabel: "Move to Trash", cancelLabel: "Cancel" },
    );
    if (!ok) return;
    try {
      const r = await trashNodes([selected.id]);
      await message(`Moved ${r.count} item to the Trash (${humanBytes(r.freed)}).`, { title: "Unshift" });
      onTrashed();
    } catch (e) {
      await message(String(e), { title: "Could not move to Trash", kind: "error" });
    }
  }

  return (
    <div className="col">
      <h2>{hovered ? "Hovered" : "Current folder"}</h2>
      <b>{node.name}</b>
      {"path" in node && <div className="mono muted">{node.path}</div>}
      <div className="stat"><span>Size</span><b>{humanBytes(node.size)}</b></div>
      {share !== null && <div className="stat"><span>Of parent</span><b>{share.toFixed(1)}%</b></div>}
      {node.dir && <div className="stat"><span>Items</span><b>{humanCount(node.items)}</b></div>}
      {node.error && <div className="note">Some of this folder could not be read.</div>}
      {!hovered && selected && selected.ancestors.length > 0 && (
        <div className="row">
          <button onClick={() => reveal(selected.id)}>Reveal in Finder</button>
          <button className="danger" onClick={doTrash}>Move to Trash…</button>
        </div>
      )}
    </div>
  );
}
