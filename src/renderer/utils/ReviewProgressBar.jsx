import React from 'react';
import '../styles.css';

/**
 * ReviewProgressBar - Displays a color-coded progress bar for file reviews
 * Shows counts of red/yellow/green statuses
 */
function ReviewProgressBar({ red = 0, yellow = 0, green = 0, total = null, showCounts = true }) {
  // Calculate total if not provided
  const computedTotal = total !== null ? total : (red + yellow + green);

  // Don't render if no data
  if (computedTotal === 0) {
    return null;
  }

  return (
    <div className="file-review-progress">
      <div className="progress-row" title={`Read: ${green} green, ${yellow} yellow, ${red} red`}>
        <span className="progress-label">Read</span>
        <div className="progress-bar">
          {red > 0 && (
            <div className="progress-segment red" style={{ width: `${(red / computedTotal) * 100}%` }} />
          )}
          {yellow > 0 && (
            <div className="progress-segment yellow" style={{ width: `${(yellow / computedTotal) * 100}%` }} />
          )}
          {green > 0 && (
            <div className="progress-segment green" style={{ width: `${(green / computedTotal) * 100}%` }} />
          )}
        </div>
        {showCounts && (
          <span className="progress-counts">{green}/{computedTotal}</span>
        )}
      </div>
    </div>
  );
}

export default ReviewProgressBar;
