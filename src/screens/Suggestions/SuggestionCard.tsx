import { Badge, Button, Card, Mono } from "../../components/ui";
import GuardIcon from "../../components/GuardIcon";
import { isProtected } from "../../lib/bin";
import { humanBytes } from "../../lib/format";
import type { Suggestion } from "../../lib/types";
import { startBinDrag } from "../Breakdown/BinBar";
import styles from "./Suggestions.module.css";

type SuggestionCardProps = {
  suggestion: Suggestion;
  inBin: boolean;
  onToggle: () => void;
};

const parentPath = (path: string) => path.slice(0, path.lastIndexOf("/")) || "/";

export default function SuggestionCard({ suggestion, inBin, onToggle }: SuggestionCardProps) {
  const { id, name, path, size, reason, safety, guard } = suggestion;
  const locked = isProtected(suggestion);
  return (
    <Card className={styles.card} draggable={!locked} onDragStart={locked ? undefined : (e) => startBinDrag(e, id)}>
      <div className={styles.text}>
        <div className={styles.title}>
          <div className={styles.name}>{name}</div>
          {!locked && <GuardIcon guard={guard} />}
        </div>
        <Mono className={styles.path}>{parentPath(path)}</Mono>
        <div className={styles.reason}>{reason}</div>
      </div>
      <div className={styles.side}>
        <Mono className={styles.size}>{humanBytes(size)}</Mono>
        {locked ? (
          <Badge tone="keep" title={guard?.reason}>
            Protected
          </Badge>
        ) : (
          (safety === "safe" || safety === "review") && (
            <Button size="sm" variant={inBin ? "primary" : "outline"} onClick={onToggle}>
              {inBin ? "Added" : "Add to bin"}
            </Button>
          )
        )}
      </div>
    </Card>
  );
}
