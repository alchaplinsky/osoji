import { PanelRightClose, PanelRightOpen } from "lucide-react";
import { IconButton } from "../../components/ui";

type ListToggleProps = { open: boolean; onToggle: () => void; className?: string };

/** Shows or hides the list panel. One fixed-size button that sits in the panel's header while open and in the chart strip while hidden. */
export default function ListToggle({ open, onToggle, className }: ListToggleProps) {
  const label = open ? "Hide list" : "Show list";
  return (
    <IconButton title={label} aria-label={label} aria-pressed={open} onClick={onToggle} className={className}>
      {open ? <PanelRightClose size={16} /> : <PanelRightOpen size={16} />}
    </IconButton>
  );
}
