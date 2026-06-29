import { useState } from 'react';
import { LucideIcon } from './LucideIcon';

interface CopyButtonProps {
  /** Text to copy to the clipboard */
  text: string;
  /** Optional extra class names */
  className?: string;
  /** Optional text label shown next to the icon */
  label?: string;
}

/** A small copy-to-clipboard button with transient "copied" feedback. */
export function CopyButton({ text, className = '', label }: CopyButtonProps) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    if (!text) return;
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (e) {
      console.error('Failed to copy message:', e);
    }
  };

  return (
    <button
      type="button"
      className={`copy-msg-btn ${copied ? 'copied' : ''} ${className}`.trim()}
      onClick={handleCopy}
      title={copied ? 'Copied!' : 'Copy'}
    >
      <LucideIcon
        name={copied ? 'check' : 'copy'}
        size={14}
        color={copied ? 'var(--accent-primary)' : 'var(--text-tertiary)'}
      />
      {label && <span>{copied ? 'Copied!' : label}</span>}
    </button>
  );
}
