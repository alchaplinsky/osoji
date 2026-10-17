import Toolbar, { ToolbarTitle } from "../../components/Toolbar";
import { Eyebrow, SegmentedControl } from "../../components/ui";
import { LAYOUT_NAMES, LAYOUTS, THEME_NAMES, THEMES } from "../../design/theme";
import { useAppVersion } from "../../hooks/useAppVersion";
import { useSettings } from "../../hooks/useSettings";
import { useTheme } from "../../hooks/useTheme";
import ExtensionCard from "./ExtensionCard";
import SettingGroup, { SettingRow } from "./SettingGroup";
import ThemeCard from "./ThemeCard";
import styles from "./Settings.module.css";

const LAYOUT_OPTIONS = LAYOUT_NAMES.map((l) => ({ value: l, label: LAYOUTS[l].label }));

const SOON_EXTENSIONS = [
  { title: "Duplicates", desc: "Find identical files scattered around the disk and keep just one.", soon: true },
  { title: "Privacy cleanup", desc: "Clear browser history, cookies and recent-item lists in one go.", soon: true },
];

export default function SettingsScreen() {
  const { theme, layout, setTheme, setLayout } = useTheme();
  const { confirmBig, setConfirmBig, watchers, setWatchers } = useSettings();
  const version = useAppVersion();

  return (
    <section className={styles.screen}>
      <Toolbar>
        <ToolbarTitle>Settings</ToolbarTitle>
      </Toolbar>
      <div className={styles.body}>
        <div>
          <Eyebrow className={styles.heading}>Layout</Eyebrow>
          <SegmentedControl options={LAYOUT_OPTIONS} value={layout} onChange={setLayout} size="md" />
        </div>
        <div>
          <Eyebrow className={styles.heading}>Appearance</Eyebrow>
          <div role="radiogroup" className={styles.themes}>
            {THEME_NAMES.map((name) => (
              <ThemeCard key={name} meta={THEMES[name]} active={name === theme} onSelect={() => setTheme(name)} />
            ))}
          </div>
        </div>
        <SettingGroup title="Safety">
          <SettingRow
            label="Move items to Trash instead of deleting"
            desc="Everything you sweep can be put back until you empty the Trash."
            checked
            title="Unshift never deletes permanently"
          />
          <SettingRow
            label="Ask before removing anything over 1 GB"
            desc="An extra confirmation for large items."
            checked={confirmBig}
            onChange={setConfirmBig}
          />
        </SettingGroup>
        <div>
          <Eyebrow className={styles.extHeading}>Extensions</Eyebrow>
          <div className={styles.intro}>Add capabilities as you need them. Anything you turn on appears in the sidebar.</div>
          <div className={styles.grid}>
            <ExtensionCard
              title="Watchers"
              desc="Keep an eye on folders that tend to grow and get a nudge when one passes a limit you set."
              on={watchers}
              onToggle={() => setWatchers(!watchers)}
            />
            {SOON_EXTENSIONS.map((e) => (
              <ExtensionCard key={e.title} {...e} />
            ))}
          </div>
        </div>
        <div className={styles.footer}>Unshift {version}</div>
      </div>
    </section>
  );
}
