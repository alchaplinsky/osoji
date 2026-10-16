import type { ReactNode } from "react";
import { Eyebrow } from "../ui";
import styles from "./NavSection.module.css";

type NavSectionProps = { title?: ReactNode; children: ReactNode };

export default function NavSection({ title, children }: NavSectionProps) {
  return (
    <nav className={styles.section}>
      {title && <Eyebrow className={styles.title}>{title}</Eyebrow>}
      {children}
    </nav>
  );
}
