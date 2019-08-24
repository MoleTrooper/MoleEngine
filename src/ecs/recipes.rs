#[macro_export]
macro_rules! recipes {
    ($( $type_name:ident, )+) => {
        #[derive(serde::Deserialize, serde::Serialize)]
        pub enum Recipes {
            $($type_name($type_name),)*
        }

        impl moleengine::ecs::space::DeserializeRecipes for Recipes {
            fn deserialize_into_space<'a, 'de, D>(
                deserializer: D,
                space: &'a mut ecs::Space,
            ) -> Result<(), D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct RecipeVisitor<'a>(&'a mut ecs::Space);

                impl<'a, 'de> serde::de::Visitor<'de> for RecipeVisitor<'a> {
                    type Value = ();

                    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                        formatter.write_str("A list of ObjectRecipes")
                    }

                    fn visit_seq<S>(self, mut seq: S) -> Result<(), S::Error>
                    where
                        S: serde::de::SeqAccess<'de>,
                    {
                        while let Some(value) = seq.next_element()? {
                            match value {
                                $(Recipes::$type_name(r) => self.0.spawn(r),)*
                            }
                        }

                        Ok(())
                    }
                }

                deserializer.deserialize_seq(RecipeVisitor(space))
            }
        }
    }
}