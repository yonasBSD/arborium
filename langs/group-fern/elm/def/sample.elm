module Main exposing (main)

import Html exposing (Html, div, text, button)
import Html.Events exposing (onClick)


type alias Model =
    { count : Int }


type Msg
    = Increment
    | Decrement


update : Msg -> Model -> Model
update msg model =
    case msg of
        Increment ->
            { model | count = model.count + 1 }

        Decrement ->
            { model | count = model.count - 1 }
