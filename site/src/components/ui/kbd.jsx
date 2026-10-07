// shadcn/ui Kbd (MIT), adapted to JavaScript. See the adjacent LICENSE.
import React from 'react';

function Kbd({ className = '', ...props }) {
  return (
    <kbd
      data-slot="kbd"
      className={`bg-muted text-muted-foreground pointer-events-none inline-flex h-5 w-fit min-w-5 select-none items-center justify-center gap-1 rounded-sm px-1 font-sans text-xs font-medium [&_svg:not([class*='size-'])]:size-3 ${className}`}
      {...props}
    />
  );
}

function KbdGroup({ className = '', ...props }) {
  return <kbd data-slot="kbd-group" className={`inline-flex items-center gap-1 ${className}`} {...props} />;
}

export { Kbd, KbdGroup };
