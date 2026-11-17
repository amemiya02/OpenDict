/** The element id of a Dictionary's section on the page, which its notch links to. */
export function sectionId(dictionaryId: string): string {
  return `dictionary-${dictionaryId}`;
}
