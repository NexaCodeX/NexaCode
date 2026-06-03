/* eslint-disable @typescript-eslint/no-explicit-any */
import { useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import rehypeHighlight from 'rehype-highlight';
import { LucideIcon } from './LucideIcon';

// Code block with copy button (hover to show)
function CodeBlock({ children, className }: { children: React.ReactNode; className?: string }) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    // Extract text content from the code element
    const codeElement = (children as any)?.props?.children;
    let codeText = '';
    if (typeof codeElement === 'string') {
      codeText = codeElement;
    } else if (Array.isArray(codeElement)) {
      codeText = codeElement.map((c: any) => (typeof c === 'string' ? c : '')).join('');
    }
    await navigator.clipboard.writeText(codeText);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  // Extract language from className (e.g. "language-python")
  const language = className?.replace(/language-/, '') || '';

  return (
    <div className="code-block-wrapper">
      {language && (
        <div className="code-block-header">
          <span className="code-language">{language}</span>
          <button className="copy-button" onClick={handleCopy}>
            {copied ? (
              <LucideIcon name="check" size={14} color="var(--accent-primary)" />
            ) : (
              <LucideIcon name="copy" size={14} color="var(--text-tertiary)" />
            )}
            <span>{copied ? 'Copied!' : 'Copy'}</span>
          </button>
        </div>
      )}
      {!language && (
        <button className="code-block-copy-float" onClick={handleCopy}>
          {copied ? (
            <LucideIcon name="check" size={14} color="var(--accent-primary)" />
          ) : (
            <LucideIcon name="copy" size={14} color="var(--text-tertiary)" />
          )}
        </button>
      )}
      <pre className="code-block">
        <code className={className}>{children}</code>
      </pre>
    </div>
  );
}

// Shared markdown component config to eliminate duplication
const markdownComponents = {
  code({ className, children, ...props }: any) {
    const isInline = !className && typeof children === 'string' && !children.includes('\n');
    if (isInline) {
      return (
        <code className="inline-code" {...props}>
          {children}
        </code>
      );
    }
    return (
      <code className={className} {...props}>
        {children}
      </code>
    );
  },
  pre({ children }: any) {
    return <CodeBlock>{children}</CodeBlock>;
  },
  p({ children }: any) {
    return <p className="markdown-paragraph">{children}</p>;
  },
  ul({ children }: any) {
    return <ul className="markdown-list">{children}</ul>;
  },
  ol({ children }: any) {
    return <ol className="markdown-list-ordered">{children}</ol>;
  },
  li({ children }: any) {
    return <li className="markdown-list-item">{children}</li>;
  },
  h1({ children }: any) {
    return <h1 className="markdown-h1">{children}</h1>;
  },
  h2({ children }: any) {
    return <h2 className="markdown-h2">{children}</h2>;
  },
  h3({ children }: any) {
    return <h3 className="markdown-h3">{children}</h3>;
  },
  a({ href, children }: any) {
    return (
      <a className="markdown-link" href={href} target="_blank" rel="noopener noreferrer">
        {children}
      </a>
    );
  },
  blockquote({ children }: any) {
    return <blockquote className="markdown-blockquote">{children}</blockquote>;
  },
  strong({ children }: any) {
    return <strong className="markdown-strong">{children}</strong>;
  },
  em({ children }: any) {
    return <em className="markdown-italic">{children}</em>;
  },
  table({ children }: any) {
    return (
      <div className="markdown-table-wrapper">
        <table className="markdown-table">{children}</table>
      </div>
    );
  },
  th({ children }: any) {
    return <th className="markdown-th">{children}</th>;
  },
  td({ children }: any) {
    return <td className="markdown-td">{children}</td>;
  },
  hr() {
    return <hr className="markdown-hr" />;
  },
  img({ src, alt }: any) {
    return <img className="markdown-image" src={src} alt={alt} />;
  },
};

interface MarkdownRendererProps {
  content: string;
}

export function MarkdownRenderer({ content }: MarkdownRendererProps) {
  const [thinkingOpen, setThinkingOpen] = useState(false);

  // Parse thinking blocks
  const thinkingRegex = /\[THINKING\]([\s\S]*?)\[\/THINKING\]/g;
  const thinkingMatches = [...content.matchAll(thinkingRegex)];
  const thinkingContent = thinkingMatches.map((m) => m[1]).join('');
  const mainContent = content.replace(thinkingRegex, '').trim();

  // Auto-expand thinking block when there's no main content (still thinking)
  const isThinkingComplete = mainContent.length > 0;
  const showThinkingContent = !isThinkingComplete || thinkingOpen;

  return (
    <div className="markdown-content">
      {thinkingContent && (
        <div className={`thinking-block ${showThinkingContent ? 'open' : ''}`}>
          <button
            className="thinking-header"
            onClick={() => setThinkingOpen(!thinkingOpen)}
          >
            <span>Thinking</span>
            <LucideIcon
              name={showThinkingContent ? 'chevron-down' : 'chevron-right'}
              size={14}
              color="#9C9B99"
            />
          </button>
          <div className={`thinking-content ${showThinkingContent ? 'open' : ''}`}>
            <div className="thinking-content-inner">
              <ReactMarkdown
                remarkPlugins={[remarkGfm]}
                rehypePlugins={[rehypeHighlight]}
                components={markdownComponents}
              >
                {thinkingContent}
              </ReactMarkdown>
            </div>
          </div>
        </div>
      )}

      {mainContent && (
        <ReactMarkdown
          remarkPlugins={[remarkGfm]}
          rehypePlugins={[rehypeHighlight]}
          components={markdownComponents}
        >
          {mainContent}
        </ReactMarkdown>
      )}
    </div>
  );
}
