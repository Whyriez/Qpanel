import React from 'react';

export interface PulseStripProps {
  data?: number[];
  hasError?: boolean;
  className?: string;
  width?: number;
  height?: number;
}

export const PulseStrip: React.FC<PulseStripProps> = ({
  data = [12, 18, 25, 20, 32, 45, 38, 28, 30, 42, 55, 60, 48, 52, 65],
  hasError = false,
  className = '',
  width = 120,
  height = 20,
}) => {
  const max = Math.max(...data, 1);
  const min = Math.min(...data, 0);
  const range = max - min || 1;

  // Build SVG path
  const points = data.map((val, idx) => {
    const x = (idx / (data.length - 1)) * width;
    const y = height - ((val - min) / range) * (height - 4) - 2;
    return `${x.toFixed(1)},${y.toFixed(1)}`;
  });

  const pathD = `M ${points.join(' L ')}`;
  const color = hasError ? 'var(--danger)' : 'var(--primary)';

  return (
    <div className={`relative inline-flex items-center ${className}`}>
      <svg
        width={width}
        height={height}
        className="overflow-visible"
        aria-label="Activity pulse sparkline"
      >
        <path
          d={pathD}
          fill="none"
          stroke={color}
          strokeWidth="1.75"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
        {/* Pulsing latest point */}
        {points.length > 0 && (
          <circle
            cx={points[points.length - 1].split(',')[0]}
            cy={points[points.length - 1].split(',')[1]}
            r="2.5"
            fill={color}
            className="animate-pulse"
          />
        )}
      </svg>
    </div>
  );
};
