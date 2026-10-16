import { Badge, Card } from "../../components/ui";
import styles from "./ExtensionCard.module.css";

type ExtensionCardProps = { title: string; desc: string; soon?: boolean };

export default function ExtensionCard({ title, desc, soon }: ExtensionCardProps) {
  return (
    <Card className={styles.card}>
      <div className={styles.head}>
        <div className={styles.title}>{title}</div>
        {soon && <Badge variant="outline">SOON</Badge>}
      </div>
      <div className={styles.desc}>{desc}</div>
    </Card>
  );
}
