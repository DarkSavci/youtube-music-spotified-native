package dev.darksavci.spotified.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.darksavci.spotified.R
import dev.darksavci.spotified.ui.theme.LocalPalette
import dev.darksavci.spotified.ui.theme.Type

/**
 * A field to search with: a pill in the theme's own colours, one height and one
 * colour whatever it holds. The clear button's place is always kept, so nothing
 * moves when it comes; at the pill's height, Compose still answers a touch on it
 * out to the 48 dp a finger needs. Anywhere on the pill puts the cursor in it.
 */
@Composable
fun SearchField(
    value: String,
    onValueChange: (String) -> Unit,
    modifier: Modifier = Modifier,
    placeholder: String = "What do you want to listen to?",
    focusRequester: FocusRequester = remember { FocusRequester() },
    onFocusChanged: (Boolean) -> Unit = {},
    onSearch: (() -> Unit)? = null,
    onClear: () -> Unit = { onValueChange("") },
) {
    val p = LocalPalette.current
    var focused by remember { mutableStateOf(false) }
    val shape = CircleShape
    Row(
        modifier.height(SEARCH_FIELD_HEIGHT)
            .background(p.surface, shape)
            .border(1.dp, if (focused) p.secondary.copy(alpha = 0.5f) else Color.Transparent, shape)
            .clickable(remember { MutableInteractionSource() }, indication = null, onClickLabel = null, role = null) {
                runCatching { focusRequester.requestFocus() }
            }
            .padding(start = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Glyph(R.drawable.ic_search, null, tint = p.secondary, size = 20.dp)
        Spacer(Modifier.width(10.dp))
        Box(Modifier.weight(1f).fillMaxHeight(), contentAlignment = Alignment.CenterStart) {
            if (value.isEmpty()) {
                Text(placeholder, style = Type.body, color = p.secondary, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
            BasicTextField(
                value, onValueChange, singleLine = true,
                textStyle = Type.body.copy(color = p.text),
                cursorBrush = SolidColor(p.accent),
                keyboardOptions = KeyboardOptions(imeAction = if (onSearch != null) ImeAction.Search else ImeAction.Done),
                keyboardActions = KeyboardActions(onSearch = { onSearch?.invoke() }),
                modifier = Modifier.fillMaxWidth().focusRequester(focusRequester).onFocusChanged {
                    focused = it.isFocused
                    onFocusChanged(it.isFocused)
                },
            )
        }
        Box(Modifier.size(SEARCH_FIELD_HEIGHT), contentAlignment = Alignment.Center) {
            if (value.isNotEmpty()) {
                Box(
                    Modifier.size(SEARCH_FIELD_HEIGHT).clip(CircleShape)
                        .tapRow(onClear, role = Role.Button, onClickLabel = "Clear"),
                    contentAlignment = Alignment.Center,
                ) { Glyph(R.drawable.ic_x, "Clear", tint = p.secondary, size = 18.dp) }
            }
        }
    }
}

/** The way out of a search, beside its field: quiet words, not a button that competes with it. */
@Composable
fun SearchCancel(onClick: () -> Unit, modifier: Modifier = Modifier) {
    Text(
        "Cancel", style = Type.body, color = LocalPalette.current.secondary, maxLines = 1,
        modifier = modifier.clip(CircleShape).tapRow(onClick, role = Role.Button).padding(horizontal = 12.dp, vertical = 14.dp),
    )
}

/** Every search field's height. */
val SEARCH_FIELD_HEIGHT = 44.dp
