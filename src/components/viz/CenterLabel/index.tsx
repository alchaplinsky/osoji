import { humanBytes } from "../../../lib/format";
import { Eyebrow, Mono } from "../../ui";
import styles from "./CenterLabel.module.css";

type CenterLabelProps = {
  name: string;
  size: number;
  /** Optional third line, e.g. the share of the whole. */
  note?: string;
  fontSize: number;
};

export default function CenterLabel({ name, size, note, fontSize }: CenterLabelProps) {
  return (
    <div className={styles.center}>
      <Mono numeral size={fontSize}>
        {humanBytes(size)}
      </Mono>
      <Eyebrow className={styles.name}>{name}</Eyebrow>
      {note && <Mono className={styles.note}>{note}</Mono>}
    </div>
  );
}
