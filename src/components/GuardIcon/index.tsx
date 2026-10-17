import { Shield, TriangleAlert } from "lucide-react";
import type { Guard } from "../../lib/types";
import { cx } from "../ui";
import styles from "./GuardIcon.module.css";

const SIZE = 12;

type GuardIconProps = { guard?: Guard | null; className?: string };

/** A shield for protected items and a warning triangle for risky ones; the policy reason shows on hover. */
export default function GuardIcon({ guard, className }: GuardIconProps) {
  if (guard?.kind !== "protected" && guard?.kind !== "risky") return null;
  const Glyph = guard.kind === "protected" ? Shield : TriangleAlert;
  return (
    <span className={cx(styles.icon, styles[guard.kind], className)} title={guard.reason} role="img" aria-label={guard.reason}>
      <Glyph size={SIZE} />
    </span>
  );
}
