import { cn } from "@/lib/utils";

// The brand symbol. While `speaking`, its three bars rise and fall like a
// voice level; the blue dot stays still.
export function YuAgentMark({ speaking = false, className }: { speaking?: boolean; className?: string }) {
  return (
    <svg viewBox="0 0 620 600" aria-hidden="true" className={cn("yu-agent-mark", speaking && "is-speaking", className)}>
      <line x1="64" y1="514" x2="298" y2="70" pathLength={100} />
      <line x1="230" y1="514" x2="381" y2="226" pathLength={100} />
      <line x1="388" y1="514" x2="483" y2="334" pathLength={100} />
      <circle cx="553" cy="520" r="39" />
    </svg>
  );
}
