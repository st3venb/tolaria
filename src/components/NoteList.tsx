import { useState, useMemo, useCallback, memo } from 'react'
import { Virtuoso } from 'react-virtuoso'
import type { VaultEntry, SidebarSelection, ModifiedFile } from '../types'
import { cn } from '@/lib/utils'
import { Input } from '@/components/ui/input'
import {
  MagnifyingGlass, Plus, Wrench, Flask, Target, ArrowsClockwise,
  Users, CalendarBlank, Tag, FileText, CaretDown, CaretRight,
} from '@phosphor-icons/react'
import type { ComponentType, SVGAttributes } from 'react'
import { getTypeColor, getTypeLightColor } from '../utils/typeColors'

const TYPE_ICON_MAP: Record<string, ComponentType<SVGAttributes<SVGSVGElement>>> = {
  Project: Wrench,
  Experiment: Flask,
  Responsibility: Target,
  Procedure: ArrowsClockwise,
  Person: Users,
  Event: CalendarBlank,
  Topic: Tag,
}

function getTypeIcon(isA: string | null): ComponentType<SVGAttributes<SVGSVGElement>> {
  return (isA && TYPE_ICON_MAP[isA]) || FileText
}

interface NoteListProps {
  entries: VaultEntry[]
  selection: SidebarSelection
  selectedNote: VaultEntry | null
  allContent: Record<string, string>
  modifiedFiles?: ModifiedFile[]
  onSelectNote: (entry: VaultEntry) => void
  onCreateNote: () => void
}

interface RelationshipGroup {
  label: string
  entries: VaultEntry[]
}

function relativeDate(ts: number | null): string {
  if (!ts) return ''
  const now = Math.floor(Date.now() / 1000)
  const diff = now - ts
  if (diff < 0) {
    const date = new Date(ts * 1000)
    return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
  }
  if (diff < 60) return 'just now'
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`
  if (diff < 604800) return `${Math.floor(diff / 86400)}d ago`
  const date = new Date(ts * 1000)
  return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
}

function getDisplayDate(entry: VaultEntry): number | null {
  return entry.modifiedAt ?? entry.createdAt
}

function refsMatch(refs: string[], entry: VaultEntry): boolean {
  const stem = entry.path.replace(/^.*\/Laputa\//, '').replace(/\.md$/, '')
  const fileStem = entry.filename.replace(/\.md$/, '')
  return refs.some((ref) => {
    const raw = ref.replace(/^\[\[/, '').replace(/\]\]$/, '')
    const inner = raw.split('|')[0]
    return inner === stem || inner.split('/').pop() === fileStem
  })
}

function resolveRefs(refs: string[], entries: VaultEntry[]): VaultEntry[] {
  return refs
    .map((ref) => {
      // Strip [[ ]] and remove alias (|display text) if present
      const raw = ref.replace(/^\[\[/, '').replace(/\]\]$/, '')
      const inner = raw.split('|')[0]
      return entries.find((e) => {
        const stem = e.path.replace(/^.*\/Laputa\//, '').replace(/\.md$/, '')
        if (stem === inner) return true
        const fileStem = e.filename.replace(/\.md$/, '')
        if (fileStem === inner.split('/').pop()) return true
        return false
      })
    })
    .filter((e): e is VaultEntry => e !== undefined)
}

function sortByModified(a: VaultEntry, b: VaultEntry): number {
  return (getDisplayDate(b) ?? 0) - (getDisplayDate(a) ?? 0)
}

function findBacklinks(entity: VaultEntry, allEntries: VaultEntry[], allContent: Record<string, string>): VaultEntry[] {
  const stem = entity.filename.replace(/\.md$/, '')
  const pathStem = entity.path.replace(/^.*\/Laputa\//, '').replace(/\.md$/, '')
  const targets = [entity.title, ...entity.aliases]

  return allEntries.filter((e) => {
    if (e.path === entity.path) return false
    const content = allContent[e.path]
    if (!content) return false
    for (const t of targets) {
      if (content.includes(`[[${t}]]`)) return true
    }
    if (content.includes(`[[${stem}]]`)) return true
    if (content.includes(`[[${pathStem}]]`)) return true
    if (content.includes(`[[${pathStem}|`)) return true
    return false
  })
}

// Priority ordering for relationship groups (lower = first).
// Keys not listed here appear alphabetically after these.
const GROUP_PRIORITY: Record<string, number> = {
  'Has': 0,
  'Children': 1,
  'Events': 2,
  'Topics': 3,
}
const BACKLINKS_KEY = 'Backlinks'

function addGroup(
  groups: RelationshipGroup[],
  label: string,
  entries: VaultEntry[],
  seen: Set<string>,
) {
  const unseen = entries.filter((e) => !seen.has(e.path))
  if (unseen.length > 0) {
    groups.push({ label, entries: unseen })
    unseen.forEach((e) => seen.add(e.path))
  }
}

function buildRelationshipGroups(
  entity: VaultEntry,
  allEntries: VaultEntry[],
  allContent: Record<string, string>,
): RelationshipGroup[] {
  const groups: RelationshipGroup[] = []
  const seen = new Set<string>([entity.path])
  const rels = entity.relationships ?? {}

  // 1. "Has" — from the entity's own relationships map
  const hasRefs = rels['Has'] ?? []
  if (hasRefs.length > 0) {
    addGroup(groups, 'Has', resolveRefs(hasRefs, allEntries).sort(sortByModified), seen)
  }

  // 2. Children — entries whose belongsTo points to this entity (reverse lookup, excluding events)
  const children = allEntries
    .filter((e) => !seen.has(e.path) && e.isA !== 'Event' && refsMatch(e.belongsTo, entity))
    .sort(sortByModified)
  addGroup(groups, 'Children', children, seen)

  // 3. Events — entities of type Event that reference this entity via belongsTo/relatedTo
  const events = allEntries
    .filter(
      (e) =>
        !seen.has(e.path) &&
        e.isA === 'Event' &&
        (refsMatch(e.belongsTo, entity) || refsMatch(e.relatedTo, entity))
    )
    .sort(sortByModified)
  addGroup(groups, 'Events', events, seen)

  // 4. "Topics" — from the entity's own relationships map
  const topicRefs = rels['Topics'] ?? []
  if (topicRefs.length > 0) {
    addGroup(groups, 'Topics', resolveRefs(topicRefs, allEntries).sort(sortByModified), seen)
  }

  // 5. All other generic relationship fields (alphabetically)
  const handledKeys = new Set(['Has', 'Topics'])
  const otherKeys = Object.keys(rels)
    .filter((k) => !handledKeys.has(k))
    .sort((a, b) => a.localeCompare(b))
  for (const key of otherKeys) {
    const refs = rels[key]
    if (refs && refs.length > 0) {
      addGroup(groups, key, resolveRefs(refs, allEntries).sort(sortByModified), seen)
    }
  }

  // 6. Referenced By — entries that reference this entity via relatedTo (reverse lookup)
  const referencedBy = allEntries
    .filter((e) => !seen.has(e.path) && e.isA !== 'Event' && refsMatch(e.relatedTo, entity))
    .sort(sortByModified)
  addGroup(groups, 'Referenced By', referencedBy, seen)

  // 7. Backlinks — always last
  const backlinks = findBacklinks(entity, allEntries, allContent)
    .filter((e) => !seen.has(e.path))
    .sort(sortByModified)
  addGroup(groups, BACKLINKS_KEY, backlinks, seen)

  return groups
}

function filterEntries(entries: VaultEntry[], selection: SidebarSelection, _modifiedFiles?: ModifiedFile[]): VaultEntry[] {
  switch (selection.kind) {
    case 'filter':
      switch (selection.filter) {
        case 'all':
          return entries
        case 'favorites':
          return []
      }
      break
    case 'sectionGroup':
      return entries.filter((e) => e.isA === selection.type)
    case 'entity':
      return []
    case 'topic': {
      const topic = selection.entry
      return entries.filter((e) => refsMatch(e.relatedTo, topic))
    }
  }
}

function NoteListInner({ entries, selection, selectedNote, allContent, modifiedFiles, onSelectNote, onCreateNote }: NoteListProps) {
  const [search, setSearch] = useState('')
  const [searchVisible, setSearchVisible] = useState(false)
  const [collapsedGroups, setCollapsedGroups] = useState<Set<string>>(new Set())

  const isEntityView = selection.kind === 'entity'

  const toggleGroup = useCallback((label: string) => {
    setCollapsedGroups((prev) => {
      const next = new Set(prev)
      if (next.has(label)) next.delete(label)
      else next.add(label)
      return next
    })
  }, [])

  const entityGroups = useMemo(
    () => isEntityView ? buildRelationshipGroups(selection.entry, entries, allContent) : [],
    [isEntityView, selection, entries, allContent]
  )

  const filtered = useMemo(
    () => isEntityView ? [] : filterEntries(entries, selection, modifiedFiles),
    [entries, selection, modifiedFiles, isEntityView]
  )

  const sorted = useMemo(
    () => isEntityView ? [] : [...filtered].sort(sortByModified),
    [filtered, isEntityView]
  )

  const query = search.trim().toLowerCase()

  const searched = useMemo(
    () => query ? sorted.filter((e) => e.title.toLowerCase().includes(query)) : sorted,
    [sorted, query]
  )

  const searchedGroups = useMemo(
    () => query
      ? entityGroups
          .map((g) => ({
            ...g,
            entries: g.entries.filter((e) => e.title.toLowerCase().includes(query)),
          }))
          .filter((g) => g.entries.length > 0)
      : entityGroups,
    [entityGroups, query]
  )


  const renderItem = useCallback((entry: VaultEntry, isPinned = false) => {
    const isSelected = selectedNote?.path === entry.path && !isPinned
    const typeColor = getTypeColor(entry.isA)
    const typeLightColor = getTypeLightColor(entry.isA)
    const TypeIcon = getTypeIcon(entry.isA)
    return (
      <div
        key={entry.path}
        className={cn(
          "relative cursor-pointer border-b border-[var(--border)] transition-colors",
          isPinned && "border-l-[3px] border-l-[var(--accent-green)] bg-muted",
          isSelected && "border-l-[3px]",
          !isPinned && !isSelected && "hover:bg-muted"
        )}
        style={{
          padding: isPinned || isSelected ? '10px 16px 10px 13px' : '10px 16px',
          ...(isSelected && {
            borderLeftColor: typeColor,
            backgroundColor: typeLightColor,
          }),
        }}
        onClick={() => onSelectNote(entry)}
      >
        <TypeIcon
          width={14}
          height={14}
          className="absolute right-3 top-2.5"
          style={{ color: typeColor }}
          data-testid="type-icon"
        />
        <div className="flex items-baseline justify-between gap-2 pr-5">
          <div className={cn(
            "min-w-0 flex-1 truncate text-[13px] text-foreground",
            isSelected ? "font-semibold" : "font-medium"
          )}>
            <span className="truncate">{entry.title}</span>
          </div>
          <span className="shrink-0 whitespace-nowrap text-[11px] text-muted-foreground">
            {relativeDate(getDisplayDate(entry))}
          </span>
        </div>
        <div className="mt-0.5 text-[12px] leading-[1.5] text-muted-foreground" style={{ display: '-webkit-box', WebkitLineClamp: 2, WebkitBoxOrient: 'vertical', overflow: 'hidden' }}>
          {entry.snippet}
        </div>
      </div>
    )
  }, [selectedNote?.path, onSelectNote])

  return (
    <div className="flex flex-col overflow-hidden border-r border-border bg-card text-foreground" style={{ height: '100%' }}>
      {/* Header */}
      <div className="flex shrink-0 items-center justify-between border-b border-border px-4 py-3.5" data-tauri-drag-region style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}>
        <h3 className="m-0 min-w-0 flex-1 truncate text-[14px] font-semibold">
          {isEntityView ? selection.entry.title : 'Notes'}
        </h3>
        <div className="flex items-center gap-3" style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}>
          <button
            className="flex items-center text-muted-foreground transition-colors hover:text-foreground"
            onClick={() => { setSearchVisible(!searchVisible); if (searchVisible) setSearch('') }}
            title="Search notes"
          >
            <MagnifyingGlass size={16} />
          </button>
          <button
            className="flex items-center text-muted-foreground transition-colors hover:text-foreground"
            onClick={onCreateNote}
            title="Create new note"
          >
            <Plus size={16} />
          </button>
        </div>
      </div>

      {/* Search (toggle on icon click) */}
      {searchVisible && (
        <div className="border-b border-border px-3 py-2">
          <Input
            placeholder="Search notes..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="h-8 text-[13px]"
            autoFocus
          />
        </div>
      )}

      {/* Items */}
      <div className="flex-1 overflow-hidden" style={{ minHeight: 0 }}>
        {isEntityView ? (() => {
          const entity = selection.entry
          const entityTypeColor = getTypeColor(entity.isA)
          const entityLightColor = getTypeLightColor(entity.isA)
          const EntityIcon = getTypeIcon(entity.isA)
          return (
            <div className="h-full overflow-y-auto">
              {/* Prominent card */}
              <div
                className="relative cursor-pointer border-b border-[var(--border)]"
                style={{ backgroundColor: entityLightColor, padding: '14px 16px' }}
                onClick={() => onSelectNote(entity)}
              >
                <EntityIcon
                  width={16}
                  height={16}
                  className="absolute right-3 top-3.5"
                  style={{ color: entityTypeColor }}
                  data-testid="type-icon"
                />
                <div className="pr-6 text-[14px] font-bold" style={{ color: entityTypeColor }}>
                  {entity.title}
                </div>
                <div className="mt-1 text-[12px] leading-[1.5] opacity-80" style={{ color: entityTypeColor, display: '-webkit-box', WebkitLineClamp: 2, WebkitBoxOrient: 'vertical', overflow: 'hidden' }}>
                  {entity.snippet}
                </div>
                <div className="mt-1 text-[11px] opacity-60" style={{ color: entityTypeColor }}>
                  {relativeDate(getDisplayDate(entity))}
                </div>
              </div>

              {/* Relationship groups */}
              {searchedGroups.length === 0 ? (
                <div className="px-4 py-8 text-center text-[13px] text-muted-foreground">
                  {query ? 'No matching items' : 'No related items'}
                </div>
              ) : (
                searchedGroups.map((group) => {
                  const isCollapsed = collapsedGroups.has(group.label)
                  return (
                    <div key={group.label} className="border-t border-[var(--border-subtle)]">
                      <button
                        className="flex w-full items-center justify-between border-none bg-transparent px-4 py-2.5 pt-3 cursor-pointer"
                        onClick={() => toggleGroup(group.label)}
                      >
                        <span className="flex items-center gap-1.5">
                          <span className="text-[11px] font-semibold uppercase text-muted-foreground" style={{ letterSpacing: '0.05em' }}>
                            {group.label}
                          </span>
                          <span className="text-[10px] text-muted-foreground">{group.entries.length}</span>
                        </span>
                        {isCollapsed
                          ? <CaretRight size={12} className="text-muted-foreground" />
                          : <CaretDown size={12} className="text-muted-foreground" />
                        }
                      </button>
                      {!isCollapsed && group.entries.map((entry) => renderItem(entry))}
                    </div>
                  )
                })
              )}
            </div>
          )
        })() : (
          searched.length === 0 ? (
            <div className="px-4 py-8 text-center text-[13px] text-muted-foreground">No notes found</div>
          ) : (
            <Virtuoso
              style={{ height: '100%' }}
              totalCount={searched.length}
              initialItemCount={Math.min(searched.length, 30)}
              itemContent={(index) => {
                const entry = searched[index]
                return entry ? renderItem(entry) : null
              }}
            />
          )
        )}
      </div>
    </div>
  )
}

export const NoteList = memo(NoteListInner)
