import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { homeDir } from "../../lib/api";
import { humanBytes, humanCount, humanDuration } from "../../lib/format";
import type { ScanStatus } from "../../hooks/useScan";
import type { ProgressMsg, ScanSummary } from "../../lib/api";

type Props = {
  status: ScanStatus;
  progress: ProgressMsg | null;
  summary: ScanSummary | null;
  error: string | null;
  onStart: (root: string) => void;
  onCancel: () => void;
};

export default function ScanControls({ status, progress, summary, error, onStart, onCancel }: Props) {
  const [root, setRoot] = useState("/");

  useEffect(() => {
    homeDir().then(setRoot).catch(() => {});
  }, []);

  async function pick() {
    const dir = await open({ directory: true, multiple: false, defaultPath: root });
    if (typeof dir === "string") setRoot(dir);
  }

  const scanning = status === "scanning";
  return (
    <div className="col">
      <h1>Unshift</h1>
      <div className="row">
        <input type="text" value={root} onChange={(e) => setRoot(e.target.value)} disabled={scanning} />
        <button onClick={pick} disabled={scanning}>…</button>
      </div>
      <div className="row">
        <button onClick={() => setRoot("/")} disabled={scanning}>Disk</button>
        <button onClick={() => homeDir().then(setRoot)} disabled={scanning}>Home</button>
        <span style={{ flex: 1 }} />
        {scanning ? (
          <button className="danger" onClick={onCancel}>Cancel</button>
        ) : (
          <button className="primary" onClick={() => onStart(root)}>Scan</button>
        )}
      </div>

      {scanning && progress && (
        <div className="col">
          <div className="progress"><div /></div>
          <div className="stat"><span>Files</span><b>{humanCount(progress.files)}</b></div>
          <div className="stat"><span>Folders</span><b>{humanCount(progress.dirs)}</b></div>
          <div className="stat"><span>Size</span><b>{humanBytes(progress.bytes)}</b></div>
          <div className="stat"><span>Elapsed</span><b>{humanDuration(progress.elapsed_ms)}</b></div>
        </div>
      )}
      {status === "done" && summary && (
        <div className="col">
          <div className="stat"><span>Entries</span><b>{humanCount(summary.entries)}</b></div>
          <div className="stat"><span>Total</span><b>{humanBytes(summary.bytes)}</b></div>
          <div className="stat"><span>Scan time</span><b>{humanDuration(summary.elapsed_ms)}</b></div>
          {summary.errors > 0 && (
            <div className="note">{humanCount(summary.errors)} folders could not be read. Grant Full Disk Access to include them.</div>
          )}
          {summary.cancelled && <div className="note">Scan cancelled; results are partial.</div>}
        </div>
      )}
      {status === "error" && <div className="note" style={{ color: "var(--danger)" }}>{error}</div>}
    </div>
  );
}
