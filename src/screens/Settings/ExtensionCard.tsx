import { Badge, Button, Card, cx } from "../../components/ui";
import styles from "./ExtensionCard.module.css";

type ExtensionCardProps = {
  title: string;
  desc: string;
  soon?: boolean;
  on?: boolean;
  onToggle?: () => void;
};

export default function ExtensionCard({ title, desc, soon, on, onToggle }: ExtensionCardProps) {
  return (
    <Card className={cx(styles.card, on && styles.on)}>
      <div className={styles.head}>
        <div className={styles.title}>{title}</div>
        {soon && <Badge variant="outline">SOON</Badge>}
      </div>
      <div className={styles.desc}>{desc}</div>
      {onToggle && (
        <Button variant={on ? "primary" : "outline"} className={styles.toggle} aria-pressed={on} onClick={onToggle}>
          {on ? "Turned on" : "Turn on"}
        </Button>
      )}
    </Card>
  );
}
