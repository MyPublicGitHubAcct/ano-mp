// Visualizations made of two others: one drawn over another, or two side by
// side (PLAN.md Phase 7b, X3). Each part is the visualization as it is
// alone, with its own state; a combination only arranges where it draws.

import type { Renderer, Scene, Visualization } from "./types";

/** `top` drawn over `base`, in a layer of its own, so its fading trails fade to `base` rather than to
    the stage. */
export function overlay(base: Visualization, top: Visualization): () => Renderer {
  return () => {
    const below = base.create();
    const above = top.create();
    const layer = document.createElement("canvas");
    const layerCtx = layer.getContext("2d");
    return {
      draw(scene: Scene) {
        below.draw(scene);
        if (!layerCtx) return;
        const width = Math.max(1, Math.round(scene.width * scene.dpr));
        const height = Math.max(1, Math.round(scene.height * scene.dpr));
        if (layer.width !== width || layer.height !== height) {
          layer.width = width;
          layer.height = height;
        }
        layerCtx.setTransform(scene.dpr, 0, 0, scene.dpr, 0, 0);
        above.draw({ ...scene, ctx: layerCtx, layer: true });
        const { ctx } = scene;
        ctx.save();
        ctx.globalCompositeOperation = "screen";
        ctx.drawImage(layer, 0, 0, scene.width, scene.height);
        ctx.restore();
      },
      dispose() {
        below.dispose?.();
        above.dispose?.();
      },
    };
  };
}

/** `first` and `second` side by side, or one above the other when the stage is taller than wide. */
export function sideBySide(first: Visualization, second: Visualization): () => Renderer {
  return () => {
    const parts = [first.create(), second.create()];
    return {
      draw(scene: Scene) {
        const across = scene.width >= scene.height;
        const width = across ? scene.width / 2 : scene.width;
        const height = across ? scene.height : scene.height / 2;
        parts.forEach((part, index) => {
          const x = across ? index * width : 0;
          const y = across ? 0 : index * height;
          const { ctx } = scene;
          ctx.save();
          ctx.beginPath();
          ctx.rect(x, y, width, height);
          ctx.clip();
          ctx.translate(x, y);
          part.draw({ ...scene, width, height });
          ctx.restore();
        });
      },
      dispose() {
        for (const part of parts) part.dispose?.();
      },
    };
  };
}
