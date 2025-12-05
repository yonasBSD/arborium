                     isTightList,
                     taskListItemFromAscii,
                     taskListItemToAscii,
                     handleTaskListItem,
                     addMetaField,
                     htmlSpanLikeElements,
                     formatCode,
                     -- * TagSoup HTML handling
                     renderTags',
                     -- * File handling
                     inDirectory,
                     makeCanonical,
                     collapseFilePath,
                     filteredFilesFromArchive,
                     -- * for squashing blocks
                     blocksToInlines,
                     blocksToInlines',
                     blocksToInlinesWithSep,
                     defaultBlocksSeparator,
                     -- * Safe read
                     safeRead,
                     safeStrRead
                    ) where

import Codec.Archive.Zip
import qualified Control.Exception as E
import Control.Monad (MonadPlus (..), msum, unless)
import qualified Control.Monad.State.Strict as S
import qualified Data.ByteString.Lazy as BL
import Data.Containers.ListUtils (nubOrd)
import Data.Char (isAlpha, isLower, isSpace, isUpper, toLower, isAlphaNum,
                  generalCategory, GeneralCategory(NonSpacingMark,
                  SpacingCombiningMark, EnclosingMark, ConnectorPunctuation))
import Data.List (find, foldl', groupBy, intercalate, intersperse, union)
import qualified Data.Map as M
import Data.Maybe (mapMaybe)
import Data.Monoid (Any (..) )
import Data.Semigroup (Min (..))
import Data.Sequence (ViewL (..), ViewR (..), viewl, viewr)
import qualified Data.Set as Set
import qualified Data.Text as T
import qualified Text.Emoji as Emoji
import System.Directory
import System.FilePath (isPathSeparator, splitDirectories)
import qualified System.FilePath.Posix as Posix
import Text.HTML.TagSoup (RenderOptions (..), Tag (..), renderOptions,
                          renderTagsOptions)
import Text.Pandoc.Builder (Blocks, Inlines, ToMetaValue (..))
import qualified Text.Pandoc.Builder as B
import Data.Time
import Text.Pandoc.Asciify (toAsciiText)
import Text.Pandoc.Definition
import Text.Pandoc.Extensions (Extensions, Extension(..), extensionEnabled)
import Text.DocLayout (charWidth)
import Text.Pandoc.Walk
-- for addPandocAttributes:
import Commonmark.Pandoc (Cm(..))
import Commonmark (HasAttributes(..))

--
-- List processing
--

-- | Split list by groups of one or more sep.
splitBy :: (a -> Bool) -> [a] -> [[a]]
splitBy _ [] = []
splitBy isSep lst =
  let (first, rest) = break isSep lst
  in  first:splitBy isSep (dropWhile isSep rest)

-- | Split text by groups of one or more separator.
splitTextBy :: (Char -> Bool) -> T.Text -> [T.Text]
splitTextBy isSep t
  | T.null t = []
  | otherwise = let (first, rest) = T.break isSep t
                in  first : splitTextBy isSep (T.dropWhile isSep rest)

-- | Split text at the given widths. Note that the break points are
-- /not/ indices but text widths, which will be different for East Asian
-- characters, emojis, etc.
splitTextByIndices :: [Int] -> T.Text -> [T.Text]
splitTextByIndices ns = splitTextByRelIndices (zipWith (-) ns (0:ns)) . T.unpack
 where
  splitTextByRelIndices [] cs = [T.pack cs]
  splitTextByRelIndices (x:xs) cs =
    let (first, rest) = splitAt' x cs
     in T.pack first : splitTextByRelIndices xs rest

-- | Returns a pair whose first element is a prefix of @t@ and that has
-- width @n@, and whose second is the remainder of the string.
--
-- Note: Do *not* replace this with 'T.splitAt', which is not sensitive
-- to character widths!
splitAt' :: Int {-^ n -} -> [Char] {-^ t -} -> ([Char],[Char])
splitAt' _ []          = ([],[])
splitAt' n xs | n <= 0 = ([],xs)
splitAt' n (x:xs)      = (x:ys,zs)
  where (ys,zs) = splitAt' (n - charWidth x) xs

--
-- Text processing
--

-- | Wrap double quotes around a Text
inquotes :: T.Text -> T.Text
inquotes txt = T.cons '\"' (T.snoc txt '\"')

-- | Like @'show'@, but returns a 'T.Text' instead of a 'String'.
tshow :: Show a => a -> T.Text
tshow = T.pack . show

-- | Strip trailing newlines from string.
stripTrailingNewlines :: T.Text -> T.Text
stripTrailingNewlines = T.dropWhileEnd (== '\n')

-- | Returns 'True' for an ASCII whitespace character, viz. space,
-- carriage return, newline, and horizontal tab.
isWS :: Char -> Bool
isWS ' '  = True
isWS '\r' = True
isWS '\n' = True
