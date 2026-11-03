// URLs of the show stills on the Shows page cards. The mock API swaps the
// factory, since <img> requests never pass through its fetch patch.

/** Stills per show: 0 is the card's face, 1..3 cycle on hover. */
export const SHOW_CARD_FRAMES = 4

type Factory = (id: string, frame: number) => string

let factory: Factory = (id, frame) => `/api/shows/${encodeURIComponent(id)}/card.png?frame=${frame}`

export function showCardUrl(id: string, frame: number): string {
  return factory(id, frame)
}

export function setShowCardFactory(f: Factory) {
  factory = f
}
