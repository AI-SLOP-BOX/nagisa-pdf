import React from 'react'
import { UserAnnotation } from '../services/annotationService'

interface AnnotationOverlayItemProps {
  annotation: UserAnnotation
  isSelected: boolean
  isInlineEditing: boolean
  scaleX: number
  scaleY: number
  isDraggingCurrent: boolean
  onSelect: (id: string) => void
  onStartInlineEdit: (id: string) => void
  onEndInlineEdit: () => void
  onStartDrag: (ann: UserAnnotation, e: React.MouseEvent) => void
  onUpdateText: (id: string, text: string) => void
  onUpdateAnnotation?: (id: string, updates: Partial<UserAnnotation>) => void
  onDelete: (id: string) => void
}

export const AnnotationOverlayItem: React.FC<AnnotationOverlayItemProps> = ({
  annotation: ann,
  isSelected,
  isInlineEditing,
  scaleX,
  scaleY,
  isDraggingCurrent,
  onSelect,
  onStartInlineEdit,
  onEndInlineEdit,
  onStartDrag,
  onUpdateText,
  onUpdateAnnotation,
  onDelete,
}) => {
  const domLeft = ann.x * scaleX
  const domTop = ann.y * scaleY
  const domWidth = ann.width * scaleX
  const domHeight = ann.height * scaleY

  if (ann.type === 'text') {
    return (
      <div
        key={ann.id}
        onClick={e => {
          e.stopPropagation()
          onSelect(ann.id)
        }}
        onDoubleClick={e => {
          e.stopPropagation()
          onStartInlineEdit(ann.id)
        }}
        onMouseDown={e => {
          if (e.button === 0 && !isInlineEditing) {
            e.stopPropagation()
            onSelect(ann.id)
            onStartDrag(ann, e)
          }
        }}
        style={{
          position: 'absolute',
          left: domLeft,
          top: domTop,
          width: domWidth ? Math.max(30, domWidth) : 'auto',
          minWidth: Math.max(30, domWidth),
          minHeight: Math.max(18, domHeight),
          maxWidth: '96vw',
          padding: '2px 4px',
          boxSizing: 'border-box',
          border: isSelected
            ? '1.5px solid #2563eb'
            : (ann.text ? '1px solid transparent' : '1px dashed #cbd5e1'),
          background: ann.bgColor || (isSelected ? 'rgba(37, 99, 235, 0.05)' : 'transparent'),
          borderRadius: 4,
          cursor: isInlineEditing ? 'text' : 'move',
          userSelect: 'none',
          zIndex: isSelected ? 30 : 15,
          transition: isDraggingCurrent ? 'none' : 'box-shadow 0.12s, transform 0.15s ease-out',
          boxShadow: isSelected ? '0 0 0 2px rgba(37, 99, 235, 0.2)' : 'none',
          whiteSpace: 'pre',
          transform: ann.rotation ? `rotate(${ann.rotation}deg)` : 'none',
          transformOrigin: 'top left',
        }}
      >
        {isInlineEditing ? (
          <textarea
            autoFocus
            defaultValue={ann.text || ''}
            onChange={e => {
              onUpdateText(ann.id, e.target.value)
              // Auto-expand inline textarea height/width
              e.target.style.width = Math.max(80, e.target.scrollWidth + 10) + 'px'
            }}
            onBlur={onEndInlineEdit}
            onKeyDown={e => {
              if (e.key === 'Escape') onEndInlineEdit()
            }}
            style={{
              width: 'max-content',
              minWidth: Math.max(60, domWidth),
              minHeight: Math.max(20, (ann.fontSize || 16) * scaleY * 1.3),
              fontSize: (ann.fontSize || 16) * scaleY,
              fontFamily: ann.fontFamily || 'Noto Sans JP, Inter, sans-serif',
              fontWeight: ann.isBold ? 700 : 400,
              fontStyle: ann.isItalic ? 'italic' : 'normal',
              color: ann.color || '#0f172a',
              border: 'none',
              outline: 'none',
              background: '#ffffff',
              resize: 'both',
              padding: 0,
              margin: 0,
              lineHeight: 1.25,
              whiteSpace: 'pre',
              overflow: 'hidden',
              borderRadius: 1,
            }}
          />
        ) : (
          ann.text ? (
            <span
              style={{
                display: 'inline-block',
                fontSize: (ann.fontSize || 16) * scaleY,
                fontFamily: ann.fontFamily || 'Noto Sans JP, Inter, sans-serif',
                fontWeight: ann.isBold ? 700 : 400,
                fontStyle: ann.isItalic ? 'italic' : 'normal',
                color: ann.color || '#0f172a',
                whiteSpace: 'pre',
                lineHeight: 1.25,
              }}
            >
              {ann.text}
            </span>
          ) : (
            <span
              style={{
                display: 'inline-block',
                fontSize: (ann.fontSize || 16) * scaleY,
                fontFamily: ann.fontFamily || 'Noto Sans JP, Inter, sans-serif',
                color: '#94a3b8',
                fontStyle: 'italic',
                whiteSpace: 'pre',
                lineHeight: 1.25,
              }}
            >
              (テキストを入力)
            </span>
          )
        )}

        {/* Selection Action Badges (Rotate 90°, Delete) */}
        {isSelected && !isInlineEditing && (
          <div
            style={{
              position: 'absolute',
              top: -24,
              right: 0,
              display: 'flex',
              alignItems: 'center',
              gap: 4,
              zIndex: 40,
            }}
          >
            {/* Quick Rotate 90° button */}
            <div
              style={{
                background: '#2563eb',
                color: '#ffffff',
                borderRadius: 4,
                padding: '2px 6px',
                fontSize: 10,
                fontWeight: 600,
                cursor: 'pointer',
                boxShadow: '0 1px 4px rgba(0,0,0,0.2)',
                display: 'flex',
                alignItems: 'center',
                gap: 3,
                userSelect: 'none',
              }}
              onClick={e => {
                e.stopPropagation()
                const currentRot = ann.rotation || 0
                const nextRot = (currentRot + 90) % 360
                onUpdateAnnotation?.(ann.id, { rotation: nextRot })
              }}
              title="90度回転"
            >
              <span>↻</span>
              <span>{ann.rotation ? `${ann.rotation}°` : '回転'}</span>
            </div>

            {/* Quick Font Size - button */}
            <div
              style={{
                width: 18,
                height: 18,
                background: '#ffffff',
                border: '1px solid #cbd5e1',
                color: '#334155',
                borderRadius: 4,
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                fontSize: 11,
                fontWeight: 700,
                cursor: 'pointer',
                boxShadow: '0 1px 3px rgba(0,0,0,0.1)',
                userSelect: 'none',
              }}
              onClick={e => {
                e.stopPropagation()
                const cur = ann.fontSize || 16
                onUpdateAnnotation?.(ann.id, { fontSize: Math.max(8, cur - 2) })
              }}
              title="文字サイズ縮小"
            >
              -
            </div>

            {/* Quick Font Size + button */}
            <div
              style={{
                width: 18,
                height: 18,
                background: '#ffffff',
                border: '1px solid #cbd5e1',
                color: '#334155',
                borderRadius: 4,
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                fontSize: 11,
                fontWeight: 700,
                cursor: 'pointer',
                boxShadow: '0 1px 3px rgba(0,0,0,0.1)',
                userSelect: 'none',
              }}
              onClick={e => {
                e.stopPropagation()
                const cur = ann.fontSize || 16
                onUpdateAnnotation?.(ann.id, { fontSize: Math.min(72, cur + 2) })
              }}
              title="文字サイズ拡大"
            >
              +
            </div>

            {/* Delete button */}
            <div
              style={{
                width: 18,
                height: 18,
                background: '#ef4444',
                color: '#ffffff',
                borderRadius: '50%',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                fontSize: 11,
                fontWeight: 700,
                cursor: 'pointer',
                boxShadow: '0 1px 3px rgba(0,0,0,0.2)',
              }}
              onClick={e => {
                e.stopPropagation()
                onDelete(ann.id)
              }}
              title="削除"
            >
              ×
            </div>
          </div>
        )}
      </div>
    )
  }

  if (ann.type === 'highlight') {
    return (
      <div
        key={ann.id}
        onClick={e => {
          e.stopPropagation()
          onSelect(ann.id)
        }}
        onMouseDown={e => {
          if (e.button === 0) {
            e.stopPropagation()
            onSelect(ann.id)
            onStartDrag(ann, e)
          }
        }}
        style={{
          position: 'absolute',
          left: domLeft,
          top: domTop,
          width: Math.max(30, domWidth),
          height: Math.max(16, domHeight),
          background: 'rgba(254, 240, 138, 0.55)',
          border: isSelected ? '2px solid #eab308' : '1px dashed transparent',
          borderRadius: 3,
          cursor: 'move',
          zIndex: isSelected ? 25 : 12,
        }}
      >
        {isSelected && (
          <div
            style={{
              position: 'absolute',
              top: -10,
              right: -10,
              width: 20,
              height: 20,
              background: '#ef4444',
              color: '#ffffff',
              borderRadius: '50%',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              fontSize: 12,
              fontWeight: 700,
              cursor: 'pointer',
              boxShadow: '0 2px 6px rgba(0,0,0,0.2)',
            }}
            onClick={e => {
              e.stopPropagation()
              onDelete(ann.id)
            }}
            title="削除"
          >
            ×
          </div>
        )}
      </div>
    )
  }

  if (ann.type === 'shape') {
    const w = Math.max(40, domWidth)
    const h = Math.max(20, domHeight)
    return (
      <div
        key={ann.id}
        onClick={e => {
          e.stopPropagation()
          onSelect(ann.id)
        }}
        onMouseDown={e => {
          if (e.button === 0) {
            e.stopPropagation()
            onSelect(ann.id)
            onStartDrag(ann, e)
          }
        }}
        style={{
          position: 'absolute',
          left: domLeft,
          top: domTop,
          width: w,
          height: h,
          cursor: 'move',
          zIndex: isSelected ? 25 : 12,
        }}
      >
        <svg
          width={w}
          height={h}
          style={{ overflow: 'visible', display: 'block' }}
        >
          <rect
            x={1}
            y={1}
            width={Math.max(1, w - 2)}
            height={Math.max(1, h - 2)}
            rx={4}
            ry={4}
            fill={ann.bgColor || 'rgba(37, 99, 235, 0.08)'}
            stroke={isSelected ? '#2563eb' : (ann.color || '#3b82f6')}
            strokeWidth={isSelected ? 2.5 : 2}
            strokeDasharray={isSelected ? '4 2' : 'none'}
            vectorEffect="non-scaling-stroke"
          />
        </svg>
        {isSelected && (
          <div
            style={{
              position: 'absolute',
              top: -10,
              right: -10,
              width: 20,
              height: 20,
              background: '#ef4444',
              color: '#ffffff',
              borderRadius: '50%',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              fontSize: 12,
              fontWeight: 700,
              cursor: 'pointer',
              boxShadow: '0 2px 6px rgba(0,0,0,0.2)',
            }}
            onClick={e => {
              e.stopPropagation()
              onDelete(ann.id)
            }}
            title="削除"
          >
            ×
          </div>
        )}
      </div>
    )
  }

  return null
}
