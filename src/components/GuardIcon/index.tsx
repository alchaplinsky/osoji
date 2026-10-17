import { Shield, Trash2, TriangleAlert } from "lucide-react";
import type { Guard } from "../../lib/types";
import { cx } from "../ui";
import styles from "./GuardIcon.module.css";

const SIZE = 12;

const GLYPHS = { protected: Shield, risky: TriangleAlert, trash: Trash2 };

type GuardIconProps = { guard?: Guard | null; className?: string };

/** A shield for protected items, a warning triangle for risky ones and a bin for the Trash; the policy reason shows on hover. */
export default function GuardIcon({ guard, className }: GuardIconProps) {
  if (guard?.kind !== "protected" && guard?.kind !== "risky" && guard?.kind !== "trash") return null;
  const Glyph = GLYPHS[guard.kind];
  return (
    <span className={cx(styles.icon, styles[guard.kind], className)} title={guard.reason} role="img" aria-label={guard.reason}>
      <Glyph size={SIZE} />
    </span>
  );
}
