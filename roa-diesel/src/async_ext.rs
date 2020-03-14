use crate::{AsyncPool, Pool, WrapError};
use diesel::connection::Connection;
use diesel::helper_types::Limit;
use diesel::query_dsl::methods::{ExecuteDsl, LimitDsl, LoadQuery};
use diesel::query_dsl::RunQueryDsl;
use diesel::result::{Error as DieselError, OptionalExtension};
use roa_core::{async_trait, Context, Error, Result, State};

/// A context extension to execute diesel dsl asynchronously.
#[async_trait]
pub trait SqlQuery<Conn: 'static + Connection> {
    /// Executes the given command, returning the number of rows affected.
    ///
    /// `execute` is usually used in conjunction with [`insert_into`](../fn.insert_into.html),
    /// [`update`](../fn.update.html) and [`delete`](../fn.delete.html) where the number of
    /// affected rows is often enough information.
    ///
    /// When asking the database to return data from a query, [`load`](#method.load) should
    /// probably be used instead.
    async fn execute<E>(&self, exec: E) -> Result<usize>
    where
        E: 'static + Send + ExecuteDsl<Conn>;
    /// Executes the given query, returning a `Vec` with the returned rows.
    ///
    /// When using the query builder,
    /// the return type can be
    /// a tuple of the values,
    /// or a struct which implements [`Queryable`].
    ///
    /// When this method is called on [`sql_query`],
    /// the return type can only be a struct which implements [`QueryableByName`]
    ///
    /// For insert, update, and delete operations where only a count of affected is needed,
    /// [`execute`] should be used instead.
    ///
    /// [`Queryable`]: ../deserialize/trait.Queryable.html
    /// [`QueryableByName`]: ../deserialize/trait.QueryableByName.html
    /// [`execute`]: fn.execute.html
    /// [`sql_query`]: ../fn.sql_query.html
    ///
    async fn load_data<U, Q>(&self, query: Q) -> Result<Vec<U>>
    where
        U: 'static + Send,
        Q: 'static + Send + LoadQuery<Conn, U>;

    /// Runs the command, and returns the affected row.
    ///
    /// `Err(NotFound)` will be returned if the query affected 0 rows. You can
    /// call `.optional()` on the result of this if the command was optional to
    /// get back a `Result<Option<U>>`
    ///
    /// When this method is called on an insert, update, or delete statement,
    /// it will implicitly add a `RETURNING *` to the query,
    /// unless a returning clause was already specified.
    async fn get_result<U, Q>(&self, query: Q) -> Result<Option<U>>
    where
        U: 'static + Send,
        Q: 'static + Send + LoadQuery<Conn, U>;

    /// Runs the command, returning an `Vec` with the affected rows.
    ///
    /// This method is an alias for [`load`], but with a name that makes more
    /// sense for insert, update, and delete statements.
    ///
    /// [`load`]: #method.load
    async fn get_results<U, Q>(&self, query: Q) -> Result<Vec<U>>
    where
        U: 'static + Send,
        Q: 'static + Send + LoadQuery<Conn, U>;

    /// Attempts to load a single record.
    ///
    /// This method is equivalent to `.limit(1).get_result()`
    ///
    /// Returns `Ok(record)` if found, and `Err(NotFound)` if no results are
    /// returned. If the query truly is optional, you can call `.optional()` on
    /// the result of this to get a `Result<Option<U>>`.
    ///
    async fn first<U, Q>(&self, query: Q) -> Result<Option<U>>
    where
        U: 'static + Send,
        Q: 'static + Send + LimitDsl,
        Limit<Q>: LoadQuery<Conn, U>;
}

#[inline]
fn map_diesel_error(err: DieselError) -> Error {
    WrapError::from(err).into()
}

#[async_trait]
impl<S, Conn> SqlQuery<Conn> for Context<S>
where
    S: State + AsRef<Pool<Conn>>,
    Conn: 'static + Connection,
{
    #[inline]
    async fn execute<E>(&self, exec: E) -> Result<usize>
    where
        E: 'static + Send + ExecuteDsl<Conn>,
    {
        let conn = self.get_conn().await?;
        self.exec
            .spawn_blocking(move || ExecuteDsl::<Conn>::execute(exec, &*conn))
            .await
            .map_err(map_diesel_error)
    }

    /// Executes the given query, returning a `Vec` with the returned rows.
    ///
    /// When using the query builder,
    /// the return type can be
    /// a tuple of the values,
    /// or a struct which implements [`Queryable`].
    ///
    /// When this method is called on [`sql_query`],
    /// the return type can only be a struct which implements [`QueryableByName`]
    ///
    /// For insert, update, and delete operations where only a count of affected is needed,
    /// [`execute`] should be used instead.
    ///
    /// [`Queryable`]: ../deserialize/trait.Queryable.html
    /// [`QueryableByName`]: ../deserialize/trait.QueryableByName.html
    /// [`execute`]: fn.execute.html
    /// [`sql_query`]: ../fn.sql_query.html
    ///
    #[inline]
    async fn load_data<U, Q>(&self, query: Q) -> Result<Vec<U>>
    where
        U: 'static + Send,
        Q: 'static + Send + LoadQuery<Conn, U>,
    {
        let conn = self.get_conn().await?;
        match self.exec.spawn_blocking(move || query.load(&*conn)).await {
            Ok(data) => Ok(data),
            Err(DieselError::NotFound) => Ok(Vec::new()),
            Err(err) => Err(map_diesel_error(err)),
        }
    }

    /// Runs the command, and returns the affected row.
    ///
    /// `Err(NotFound)` will be returned if the query affected 0 rows. You can
    /// call `.optional()` on the result of this if the command was optional to
    /// get back a `Result<Option<U>>`
    ///
    /// When this method is called on an insert, update, or delete statement,
    /// it will implicitly add a `RETURNING *` to the query,
    /// unless a returning clause was already specified.
    #[inline]
    async fn get_result<U, Q>(&self, query: Q) -> Result<Option<U>>
    where
        U: 'static + Send,
        Q: 'static + Send + LoadQuery<Conn, U>,
    {
        let conn = self.get_conn().await?;
        self.exec
            .spawn_blocking(move || query.get_result(&*conn))
            .await
            .optional()
            .map_err(map_diesel_error)
    }

    /// Runs the command, returning an `Vec` with the affected rows.
    ///
    /// This method is an alias for [`load`], but with a name that makes more
    /// sense for insert, update, and delete statements.
    ///
    /// [`load`]: #method.load
    #[inline]
    async fn get_results<U, Q>(&self, query: Q) -> Result<Vec<U>>
    where
        U: 'static + Send,
        Q: 'static + Send + LoadQuery<Conn, U>,
    {
        self.load_data(query).await
    }

    /// Attempts to load a single record.
    ///
    /// This method is equivalent to `.limit(1).get_result()`
    ///
    /// Returns `Ok(record)` if found, and `Err(NotFound)` if no results are
    /// returned. If the query truly is optional, you can call `.optional()` on
    /// the result of this to get a `Result<Option<U>>`.
    ///
    #[inline]
    async fn first<U, Q>(&self, query: Q) -> Result<Option<U>>
    where
        U: 'static + Send,
        Q: 'static + Send + LimitDsl,
        Limit<Q>: LoadQuery<Conn, U>,
    {
        let conn = self.get_conn().await?;
        self.exec
            .spawn_blocking(move || query.limit(1).get_result(&*conn))
            .await
            .optional()
            .map_err(map_diesel_error)
    }
}
