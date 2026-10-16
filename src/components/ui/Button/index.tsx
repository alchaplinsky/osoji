import type { ButtonHTMLAttributes } from "react";
import { cx } from "../cx";
import styles from "./Button.module.css";

export type ButtonVariant = "primary" | "outline" | "ghost";
export type ButtonSize = "sm" | "md" | "lg" | "xl";

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: ButtonVariant;
  size?: ButtonSize;
  muted?: boolean;
};

export default function Button({ variant = "outline", size = "md", muted, className, type = "button", ...rest }: ButtonProps) {
  return <button type={type} className={cx(styles.btn, styles[variant], styles[size], muted && styles.muted, className)} {...rest} />;
}

type IconButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & { active?: boolean };

export function IconButton({ active, className, type = "button", ...rest }: IconButtonProps) {
  return <button type={type} className={cx(styles.iconBtn, active && styles.active, className)} {...rest} />;
}
