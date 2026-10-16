import { useEffect, useState } from "react";
import { isLayout, isTheme, LAYOUT_NAMES, LAYOUTS, THEME_NAMES, THEMES } from "../design/theme";
import { hueAt, neutral, shade } from "../design/palette";
import { useTheme } from "../hooks/useTheme";
import {
  Badge, Button, Card, Checkbox, Eyebrow, Icon, IconButton, Menu, Mono, Panel, ProgressBar,
  SegmentedControl, Sheet, StatTile, Toast, Toggle, type IconName,
} from "../components/ui";

const ICON_NAMES: IconName[] = ["logo", "scan", "breakdown", "largest", "suggest", "watch", "settings", "list", "more", "trash", "chevronDown", "chevronRight", "chevronRightSm", "close", "check", "checkLg"];

/** Dev-only showcase of every primitive. Open http://localhost:1420/?gallery while `vite` runs. */
export default function Gallery() {
  const { theme, layout, setTheme, setLayout } = useTheme();
  const [checked, setChecked] = useState(true);
  const [on, setOn] = useState(true);
  const [seg, setSeg] = useState("sunburst");
  const [menu, setMenu] = useState(false);
  const [sheet, setSheet] = useState(false);
  const [toast, setToast] = useState(false);

  // `?gallery&theme=glass&layout=classic` picks the appearance for screenshots.
  useEffect(() => {
    const q = new URLSearchParams(location.search);
    const t = q.get("theme");
    const l = q.get("layout");
    if (isTheme(t)) setTheme(t);
    if (isLayout(l)) setLayout(l);
    const open = q.get("open");
    if (open === "sheet") setSheet(true);
    if (open === "toast") setToast(true);
    if (open === "menu") setMenu(true);
  }, [setTheme, setLayout]);

  return (
    <div style={{ minHeight: "100%", padding: 28, display: "flex", flexDirection: "column", gap: 28, maxWidth: 900 }}>
      <Row label="Appearance">
        <SegmentedControl options={THEME_NAMES.map((t) => ({ value: t, label: THEMES[t].label }))} value={theme} onChange={setTheme} />
        <SegmentedControl options={LAYOUT_NAMES.map((l) => ({ value: l, label: LAYOUTS[l].label }))} value={layout} onChange={setLayout} size="md" />
      </Row>

      <Row label="Buttons">
        <Button variant="primary" size="xl">Scan Macintosh HD</Button>
        <Button variant="primary" size="lg">Move 42 GB to Trash</Button>
        <Button variant="primary">Clean up</Button>
        <Button variant="primary" size="sm">Add to bin</Button>
        <Button size="lg">Cancel</Button>
        <Button>Watch a folder</Button>
        <Button muted>Stop</Button>
        <Button variant="ghost" size="sm">Ghost</Button>
        <Button variant="primary" size="lg" disabled>Free up</Button>
        <IconButton><Icon name="list" /></IconButton>
        <IconButton active><Icon name="list" /></IconButton>
        <span style={{ position: "relative" }}>
          <IconButton onClick={() => setMenu((m) => !m)}><Icon name="more" /></IconButton>
          <Menu
            open={menu}
            onClose={() => setMenu(false)}
            items={[
              { label: "Scan again", hint: "⌘R", onSelect: () => {} },
              { label: "Show hidden items", hint: "⌘⇧.", onSelect: () => {} },
              { label: "Reveal in Finder", hint: "⌘⏎", onSelect: () => {} },
              { label: "Export report…", onSelect: () => {} },
            ]}
          />
        </span>
      </Row>

      <Row label="Controls">
        <SegmentedControl
          options={[{ value: "sunburst", label: "Sunburst" }, { value: "treemap", label: "Treemap" }, { value: "shelf", label: "Shelf" }, { value: "orbit", label: "Orbit" }]}
          value={seg}
          onChange={setSeg}
        />
        <Checkbox checked={checked} onChange={setChecked} aria-label="Add to bin" />
        <Toggle checked={on} onChange={setOn} aria-label="Include hidden files" />
      </Row>

      <Row label="Badges">
        <Badge tone="safe">Safe</Badge>
        <Badge tone="review">Worth a look</Badge>
        <Badge tone="keep">Keep</Badge>
        <Badge variant="outline">SOON</Badge>
        <Badge variant="count">142 GB</Badge>
        <Badge variant="count">2 over</Badge>
      </Row>

      <Row label="Text">
        <Eyebrow>Files scanned</Eyebrow>
        <Mono>/Users/alex/Library/Caches</Mono>
        <Mono size={28} numeral>612 GB</Mono>
        <Mono size={42} numeral>61%</Mono>
      </Row>

      <Row label="Progress">
        <div style={{ width: 220, display: "flex", flexDirection: "column", gap: 10 }}>
          <ProgressBar value={61} height={3} />
          <ProgressBar value={88} height={4} color="var(--review)" />
          <ProgressBar value={35} height={8} color={shade(theme, hueAt(3), 1)} />
        </div>
      </Row>

      <Row label="Surfaces">
        <StatTile label="Files scanned" value="1,284,903" style={{ width: 200 }} />
        <StatTile label="Found so far" value="612 GB" style={{ width: 200 }} />
        <Card style={{ padding: 14, width: 200 }}>Card</Card>
        <Panel style={{ padding: 14, width: 200 }}>Panel</Panel>
      </Row>

      <Row label="Chart palette">
        {[1, 2, 3].map((depth) => (
          <div key={depth} style={{ display: "flex", gap: 4 }}>
            {Array.from({ length: 10 }, (_, i) => (
              <span key={i} title={shade(theme, hueAt(i), depth)} style={{ width: 22, height: 22, borderRadius: 6, background: shade(theme, hueAt(i), depth) }} />
            ))}
            <span style={{ width: 22, height: 22, borderRadius: 6, background: neutral(theme) }} />
          </div>
        ))}
      </Row>

      <Row label="Icons">
        {ICON_NAMES.map((n) => (
          <span key={n} title={n} style={{ color: "var(--fg2)", display: "inline-flex", width: 28, height: 28, alignItems: "center", justifyContent: "center", background: "var(--card2)", borderRadius: 8 }}>
            <Icon name={n} />
          </span>
        ))}
      </Row>

      <Row label="Overlays">
        <Button onClick={() => setSheet(true)}>Open sheet</Button>
        <Button onClick={() => { setToast(true); setTimeout(() => setToast(false), 3000); }}>Show toast</Button>
      </Row>

      <Sheet
        open={sheet}
        onClose={() => setSheet(false)}
        title="Review before sweeping"
        sub="3 items · 42.6 GB will be freed on Macintosh HD"
        footer={
          <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
            <Button size="lg" onClick={() => setSheet(false)}>Cancel</Button>
            <Button variant="primary" size="lg" onClick={() => setSheet(false)}>Move 42.6 GB to Trash</Button>
          </div>
        }
      >
        <div style={{ padding: "10px 12px" }}>Caches <Badge tone="safe">Safe</Badge></div>
        <div style={{ padding: "10px 12px" }}>Xcode DerivedData <Badge tone="safe">Safe</Badge></div>
        <div style={{ padding: "10px 12px" }}>Photos Library <Badge tone="keep">Keep</Badge></div>
      </Sheet>

      {toast && <Toast title="Freed 42.6 GB" sub="Macintosh HD now has 431 GB free" />}
    </div>
  );
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <section>
      <Eyebrow style={{ marginBottom: 10 }}>{label}</Eyebrow>
      <div style={{ display: "flex", flexWrap: "wrap", alignItems: "center", gap: 12 }}>{children}</div>
    </section>
  );
}
