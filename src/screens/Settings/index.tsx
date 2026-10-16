import Toolbar, { ToolbarTitle } from "../../components/Toolbar";
import { Eyebrow, SegmentedControl } from "../../components/ui";
import { LAYOUT_NAMES, LAYOUTS, THEME_NAMES, THEMES } from "../../design/theme";
import { useTheme } from "../../hooks/useTheme";
import ThemeCard from "./ThemeCard";
import styles from "./Settings.module.css";

const LAYOUT_OPTIONS = LAYOUT_NAMES.map((l) => ({ value: l, label: LAYOUTS[l].label }));

export default function SettingsScreen() {
  const { theme, layout, setTheme, setLayout } = useTheme();

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
        <div className={styles.footer}>Unshift 0.1.0 · Up to date</div>
      </div>
    </section>
  );
}
