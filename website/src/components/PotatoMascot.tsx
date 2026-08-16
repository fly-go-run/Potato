/**
 * Potato mascot (same as logo symbol). Used in Hero and Nav.
 */
import { CatPawIcon } from "./CatPawIcon";

interface PotatoMascotProps {
  size?: number;
  className?: string;
}

export function PotatoMascot({
  size = 80,
  className = "",
}: PotatoMascotProps) {
  return <CatPawIcon size={size} className={className} />;
}
