import SettingsScreen from "../../screens/Settings";
import { SettingsProvider } from "../../hooks/useSettings";
import styles from "./BreakdownPreview.module.css";

export default function SettingsPreview() {
  return (
    <SettingsProvider>
      <div className={styles.frame}>
        <div className={styles.sidebar} />
        <main className={styles.main}>
          <SettingsScreen />
        </main>
      </div>
    </SettingsProvider>
  );
}
