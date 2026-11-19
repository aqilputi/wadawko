//! Ações GPUI do app (uma por comando). O nome completo (`transport::TogglePlay`)
//! é o id usado no keymap JSON, na paleta e no registro (`registry.rs`).
//!
//! Para acrescentar um comando: declare a ação aqui, dê um título em
//! `registry::SPECS` (aparece na paleta) e, se quiser atalho padrão, uma linha em
//! `assets/keymap-default.json`.

// ───────────────────────────── ações ─────────────────────────────

pub mod transport {
    use gpui_kit::actions;
    actions!(
        transport,
        [
            TogglePlay,
            PlayHere,
            PlayFromStart,
            GoToStart,
            Stop,
            ToggleLoop,
            ToggleMetronome,
            ToggleCountIn,
            ToggleTrainer,
            ToggleSwing,
            Straight,
            SwingOn,
            BpmUp,
            BpmDown,
            SetBpm,
            TrainerPct,
            TrainerStep,
            TrainerTarget,
            TrainerLaps,
            MetroPattern,
            MetroSubdiv,
            MetroVolume,
            MetroSound,
            ToggleCountInLoop,
            TapTempo,
            RehearsePick,
            RehearseOff,
            RehearseListen,
            ClearRegion,
            LoopSelection,
            LoopStartHere,
            LoopEndHere,
            PlayFue
        ]
    );
}
pub mod view {
    use gpui_kit::actions;
    actions!(
        view,
        [
            Grade,
            Partitura,
            Esteira,
            ToggleTheme,
            ZoomIn,
            ZoomOut,
            ZoomReset,
            RowTaller,
            RowShorter,
            RowReset,
            Director,
            ToggleSections,
            ToggleMinimap,
            ToggleFollow,
            ToggleFollowCenter,
            ToggleNav,
            ToggleLayout
        ]
    );
}
pub mod nav {
    use gpui_kit::actions;
    actions!(nav, [Editor, Pessoas, Mixer, Kit, Config]);
}
pub mod keys {
    use gpui_kit::actions;
    actions!(keys, [TogglePalette, ShowShortcuts, ReloadKeymap, ToggleVim, Cancel, ToggleRootHints, CreateKeymap, PaletteNext, PalettePrev]);
}
pub mod hints {
    use gpui_kit::actions;
    actions!(hints, [Jump, JumpTrack, JumpCells]);
}
pub mod project {
    use gpui_kit::actions;
    actions!(project, [Undo, Redo, New, Duplicate, Rename, Delete, Save, NextSong, PrevSong, ExportJson, ImportJson, ImportLibrary, ExportMp3, ExportWav, ExportMidi, CancelExport]);
}
pub mod fill {
    use gpui_kit::actions;
    actions!(fill, [Open, Every2, Every4, Offbeat, Euclid3, Euclid5, Oroshi]);
}
pub mod step {
    use gpui_kit::actions;
    actions!(step, [Step0, Step1, Step2, Step3, Step4, Step5, Step6, Step7, Step8, Step9]);
}
pub mod fx {
    use gpui_kit::actions;
    actions!(fx, [Edit, Roll, RollCresc, Flam, Later, Earlier, Cut, Softer, Exact, Clear, ToggleColumn]);
}
pub mod practice {
    use gpui_kit::actions;
    actions!(practice, [Save]);
}
pub mod section {
    use gpui_kit::actions;
    actions!(
        section,
        [Add, Clone, Remove, Copy, Cut, Paste, Group, Ungroup, RepeatUp, RepeatDown, MeasureAdd, MeasureRemove, ToggleCollapse, ExpandAll, CollapseAll, MoveUp, MoveDown, Rename, ToggleSelect]
    );
}
pub mod track {
    use gpui_kit::actions;
    actions!(track, [Add, Remove, ToggleMute, ToggleSolo, CycleInstrument, Rename, MoveUp, MoveDown, ToggleMain, ToggleDynRow, ToggleKyaiRow, Settings, CycleColor]);
}
pub mod score {
    use gpui_kit::actions;
    actions!(score, [ExportSvg, ExportPng, Print]);
}
pub mod song {
    use gpui_kit::actions;
    actions!(song, [Sub2, Sub4, Sub8]);
}
pub mod measure {
    use gpui_kit::actions;
    actions!(measure, [CycleDynamic, ToggleCresc, ToggleDim, BeatsUp, BeatsDown, TempoRall, TempoAccel, ATempo, TempoClear]);
}
pub mod grid {
    use gpui_kit::actions;
    actions!(
        grid,
        [
            NextTrackCell,
            PrevTrackCell,
            Left,
            Right,
            Up,
            Down,
            ExtendLeft,
            ExtendRight,
            ExtendUp,
            ExtendDown,
            NextBeat,
            PrevBeat,
            EndOfBeat,
            NextMeasure,
            PrevMeasure,
            MeasureStart,
            MeasureEnd,
            FirstFilled,
            SongStart,
            SongEnd,
            NextSection,
            PrevSection,
            FirstTrack,
            LastTrack,
            HalfRight,
            HalfLeft,
            DeleteCell,
            DeleteCellBack,
            DeleteMeasureRow,
            DeleteBeat,
            DeleteToMeasureEnd,
            DeleteMeasureAll,
            DeleteSectionRow,
            YankMeasureRow,
            YankBeat,
            YankSectionRow,
            PasteAfter,
            PasteBefore,
            Copy,
            Cut,
            Paste,
            DeleteSelection,
            ToggleSfz,
            Accent,
            Unaccent,
            ToggleSelectMode,
            ModeEdit,
            ModeSelect,
            MoveSelLeft,
            MoveSelRight,
            MoveSelUp,
            MoveSelDown,
            CloneSelLeft,
            CloneSelRight,
            CloneSelUp,
            CloneSelDown,
            AssignToPen,
            NextPen,
            ToggleKyaiLane,
            NextCell,
            PrevCell
        ]
    );
}
pub mod vim {
    use gpui_kit::actions;
    actions!(
        vim,
        [
            Insert,
            Append,
            InsertMeasureStart,
            AppendMeasureEnd,
            Substitute,
            Visual,
            VisualMeasure,
            Normal,
            ChangeMeasureRow,
            ChangeBeat,
            ChangeSelection,
            SwapAnchor,
            Repeat,
            PushRegister,
            SetMark,
            GotoMark,
            Search,
            SearchBack,
            SearchNext,
            SearchPrev,
            CommandLine,
            Digit0,
            Digit1,
            Digit2,
            Digit3,
            Digit4,
            Digit5,
            Digit6,
            Digit7,
            Digit8,
            Digit9
        ]
    );
}
