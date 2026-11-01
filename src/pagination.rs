use crate::{contract, ApiResponse, Error, ErrorKind, RaimsClient, RequestOptions};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::{collections::HashSet, fmt, marker::PhantomData};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    Next,
    Previous,
}

/// Lazy cursor navigation that keeps the original filters and request options.
pub struct Pager<T> {
    client: RaimsClient,
    operation_id: &'static str,
    path_params: Vec<(String, String)>,
    params: Value,
    options: RequestOptions,
    current_page: Option<Value>,
    direction: Option<Direction>,
    visited: HashSet<String>,
    marker: PhantomData<T>,
}

impl<T> fmt::Debug for Pager<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Pager")
            .field("operation_id", &self.operation_id)
            .field("started", &self.current_page.is_some())
            .finish_non_exhaustive()
    }
}

impl<T: DeserializeOwned> Pager<T> {
    pub(crate) fn new(
        client: RaimsClient,
        operation_id: &'static str,
        path_params: Vec<(String, String)>,
        params: Value,
        options: RequestOptions,
    ) -> Result<Self, Error> {
        let op = contract::operation(operation_id)?;
        if op["paginated"] != true || !params.is_object() {
            return Err(Error::validation(
                "The operation does not support cursor pagination.",
            ));
        }
        contract::validate_input(&params, &op["params_schema"])?;
        Ok(Self {
            client,
            operation_id,
            path_params,
            params,
            options,
            current_page: None,
            direction: None,
            visited: HashSet::new(),
            marker: PhantomData,
        })
    }

    /// Fetches the first page, then advances until the server reports the end.
    /// A failed or cancelled fetch leaves the cursor unchanged for a later retry.
    pub async fn next_page(&mut self) -> Result<Option<ApiResponse<T>>, Error> {
        self.fetch(Direction::Next).await
    }

    /// Fetches the previous page after a successful page has been received.
    pub async fn previous_page(&mut self) -> Result<Option<ApiResponse<T>>, Error> {
        self.fetch(Direction::Previous).await
    }

    async fn fetch(&mut self, direction: Direction) -> Result<Option<ApiResponse<T>>, Error> {
        let mut params = self.params.clone();
        let mut requested = None;
        if let Some(page) = &self.current_page {
            let (has, cursor, remove, set) = if direction == Direction::Next {
                ("has_next", "next_after", "before", "after")
            } else {
                ("has_previous", "previous_before", "after", "before")
            };
            if page.get(has).and_then(Value::as_bool) != Some(true) {
                return Ok(None);
            }
            let cursor = page
                .get(cursor)
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    Error::new(
                        ErrorKind::UnexpectedResponse,
                        "The API pagination cursor is missing or invalid.",
                    )
                })?;
            if self.direction == Some(direction) && self.visited.contains(cursor) {
                return Err(Error::new(
                    ErrorKind::UnexpectedResponse,
                    "The API repeated a pagination cursor.",
                ));
            }
            let fields = params
                .as_object_mut()
                .ok_or_else(|| Error::validation("Pagination parameters must be an object."))?;
            fields.remove(remove);
            fields.insert(set.to_owned(), Value::String(cursor.to_owned()));
            requested = Some(cursor.to_owned());
        } else if direction == Direction::Previous {
            return Ok(None);
        } else if let Some(cursor) = params.get("after").and_then(Value::as_str) {
            requested = Some(cursor.to_owned());
        }
        let paths: Vec<_> = self
            .path_params
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect();
        let response = self
            .client
            .request(self.operation_id, &paths, &params, &self.options)
            .await?;
        let page = response
            .raw
            .get("page")
            .filter(|p| p.is_object())
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::UnexpectedResponse,
                    "The API pagination metadata is missing or invalid.",
                )
                .with_response(response.last_response.clone())
            })?;
        if self.direction != Some(direction) {
            self.visited.clear();
        }
        if let Some(cursor) = requested {
            self.visited.insert(cursor);
        }
        self.direction = Some(direction);
        self.params = params;
        self.current_page = Some(page.clone());
        Ok(Some(response))
    }
}
