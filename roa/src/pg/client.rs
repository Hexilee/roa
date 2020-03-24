use crate::http::StatusCode;
use bytes::Buf;
use futures::TryStreamExt;
use tokio_postgres::types::{ToSql, Type};
use tokio_postgres::{
    CancelToken, CopyInSink, CopyOutStream, Row, RowStream, SimpleQueryMessage,
    Statement, ToStatement, Transaction, TransactionBuilder,
};

pub struct Client(pub(crate) tokio_postgres::Client);

#[derive(Debug)]
pub struct Error(tokio_postgres::Error);

impl From<tokio_postgres::Error> for Error {
    #[inline]
    fn from(err: tokio_postgres::Error) -> Self {
        Error(err)
    }
}

impl From<Error> for crate::Status {
    #[inline]
    fn from(err: tokio_postgres::Error) -> Self {
        crate::Status::new(StatusCode::INTERNAL_SERVER_ERROR, err, false)
    }
}

impl Client {
    /// Creates a new prepared statement.
    ///
    /// Prepared statements can be executed repeatedly, and may contain query parameters (indicated by `$1`, `$2`, etc),
    /// which are set when executed. Prepared statements can only be used with the connection that created them.
    #[inline]
    pub async fn prepare(&self, query: &str) -> Result<Statement, Error> {
        Ok(self.0.prepare(query).await?)
    }

    /// Like `prepare`, but allows the types of query parameters to be explicitly specified.
    ///
    /// The list of types may be smaller than the number of parameters - the types of the remaining parameters will be
    /// inferred. For example, `client.prepare_typed(query, &[])` is equivalent to `client.prepare(query)`.
    #[inline]
    pub async fn prepare_typed(
        &self,
        query: &str,
        parameter_types: &[Type],
    ) -> Result<Statement, Error> {
        Ok(self.0.prepare_typed(query, parameter_types).await?)
    }

    /// Executes a statement, returning a vector of the resulting rows.
    ///
    /// A statement may contain parameters, specified by `$n`, where `n` is the index of the parameter of the list
    /// provided, 1-indexed.
    ///
    /// The `statement` argument can either be a `Statement`, or a raw query string. If the same statement will be
    /// repeatedly executed (perhaps with different query parameters), consider preparing the statement up front
    /// with the `prepare` method.
    ///
    /// # Panics
    ///
    /// Panics if the number of parameters provided does not match the number expected.
    #[inline]
    pub async fn query<T>(
        &self,
        statement: &T,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<Vec<Row>, Error>
    where
        T: ?Sized + ToStatement,
    {
        Ok(self.0.query(statement, params).await?)
    }

    /// Executes a statement which returns a single row, returning it.
    ///
    /// Returns an error if the query does not return exactly one row.
    ///
    /// A statement may contain parameters, specified by `$n`, where `n` is the index of the parameter of the list
    /// provided, 1-indexed.
    ///
    /// The `statement` argument can either be a `Statement`, or a raw query string. If the same statement will be
    /// repeatedly executed (perhaps with different query parameters), consider preparing the statement up front
    /// with the `prepare` method.
    ///
    /// # Panics
    ///
    /// Panics if the number of parameters provided does not match the number expected.
    #[inline]
    pub async fn query_one<T>(
        &self,
        statement: &T,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<Row, Error>
    where
        T: ?Sized + ToStatement,
    {
        Ok(self.0.query_one(statement, params).await?)
    }

    /// Executes a statements which returns zero or one rows, returning it.
    ///
    /// Returns an error if the query returns more than one row.
    ///
    /// A statement may contain parameters, specified by `$n`, where `n` is the index of the parameter of the list
    /// provided, 1-indexed.
    ///
    /// The `statement` argument can either be a `Statement`, or a raw query string. If the same statement will be
    /// repeatedly executed (perhaps with different query parameters), consider preparing the statement up front
    /// with the `prepare` method.
    ///
    /// # Panics
    ///
    /// Panics if the number of parameters provided does not match the number expected.
    #[inline]
    pub async fn query_opt<T>(
        &self,
        statement: &T,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<Option<Row>, Error>
    where
        T: ?Sized + ToStatement,
    {
        Ok(self.0.query_opt(statement, params).await?)
    }

    /// The maximally flexible version of [`query`].
    ///
    /// A statement may contain parameters, specified by `$n`, where `n` is the index of the parameter of the list
    /// provided, 1-indexed.
    ///
    /// The `statement` argument can either be a `Statement`, or a raw query string. If the same statement will be
    /// repeatedly executed (perhaps with different query parameters), consider preparing the statement up front
    /// with the `prepare` method.
    ///
    /// # Panics
    ///
    /// Panics if the number of parameters provided does not match the number expected.
    ///
    /// [`query`]: #method.query
    ///
    /// # Examples
    ///
    /// If you have a type like `Vec<T>` where `T: ToSql` Rust will not know how to use it as params. To get around
    /// this the type must explicitly be converted to `&dyn ToSql`.
    ///
    /// ```no_run
    /// # async fn async_main(client: &tokio_postgres::Client) -> Result<(), tokio_postgres::Error> {
    /// use tokio_postgres::types::ToSql;
    /// use futures::{pin_mut, TryStreamExt};
    ///
    /// let params: Vec<String> = vec![
    ///     "first param".into(),
    ///     "second param".into(),
    /// ];
    /// let mut it = client.query_raw(
    ///     "SELECT foo FROM bar WHERE biz = $1 AND baz = $2",
    ///     params.iter().map(|p| p as &dyn ToSql),
    /// ).await?;
    ///
    /// pin_mut!(it);
    /// while let Some(row) = it.try_next().await? {
    ///     let foo: i32 = row.get("foo");
    ///     println!("foo: {}", foo);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub async fn query_raw<'a, T, I>(
        &self,
        statement: &T,
        params: I,
    ) -> Result<RowStream, Error>
    where
        T: ?Sized + ToStatement,
        I: IntoIterator<Item = &'a dyn ToSql>,
        I::IntoIter: ExactSizeIterator,
    {
        Ok(self.0.query_raw(statement, params).await?)
    }

    /// Executes a statement, returning the number of rows modified.
    ///
    /// A statement may contain parameters, specified by `$n`, where `n` is the index of the parameter of the list
    /// provided, 1-indexed.
    ///
    /// The `statement` argument can either be a `Statement`, or a raw query string. If the same statement will be
    /// repeatedly executed (perhaps with different query parameters), consider preparing the statement up front
    /// with the `prepare` method.
    ///
    /// If the statement does not modify any rows (e.g. `SELECT`), 0 is returned.
    ///
    /// # Panics
    ///
    /// Panics if the number of parameters provided does not match the number expected.
    #[inline]
    pub async fn execute<T>(
        &self,
        statement: &T,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<u64, Error>
    where
        T: ?Sized + ToStatement,
    {
        Ok(self.0.execute(statement, params).await?)
    }

    /// The maximally flexible version of [`execute`].
    ///
    /// A statement may contain parameters, specified by `$n`, where `n` is the index of the parameter of the list
    /// provided, 1-indexed.
    ///
    /// The `statement` argument can either be a `Statement`, or a raw query string. If the same statement will be
    /// repeatedly executed (perhaps with different query parameters), consider preparing the statement up front
    /// with the `prepare` method.
    ///
    /// # Panics
    ///
    /// Panics if the number of parameters provided does not match the number expected.
    ///
    /// [`execute`]: #method.execute
    #[inline]
    pub async fn execute_raw<'a, T, I>(
        &self,
        statement: &T,
        params: I,
    ) -> Result<u64, Error>
    where
        T: ?Sized + ToStatement,
        I: IntoIterator<Item = &'a dyn ToSql>,
        I::IntoIter: ExactSizeIterator,
    {
        Ok(self.0.execute_raw(statement, params).await?)
    }

    /// Executes a `COPY FROM STDIN` statement, returning a sink used to write the copy data.
    ///
    /// PostgreSQL does not support parameters in `COPY` statements, so this method does not take any. The copy *must*
    /// be explicitly completed via the `Sink::close` or `finish` methods. If it is not, the copy will be aborted.
    ///
    /// # Panics
    ///
    /// Panics if the statement contains parameters.
    #[inline]
    pub async fn copy_in<T, U>(&self, statement: &T) -> Result<CopyInSink<U>, Error>
    where
        T: ?Sized + ToStatement,
        U: Buf + 'static + Send,
    {
        Ok(self.0.copy_in(statement).await?)
    }

    /// Executes a `COPY TO STDOUT` statement, returning a stream of the resulting data.
    ///
    /// PostgreSQL does not support parameters in `COPY` statements, so this method does not take any.
    ///
    /// # Panics
    ///
    /// Panics if the statement contains parameters.
    #[inline]
    pub async fn copy_out<T>(&self, statement: &T) -> Result<CopyOutStream, Error>
    where
        T: ?Sized + ToStatement,
    {
        Ok(self.0.copy_out(statement).await?)
    }

    /// Executes a sequence of SQL statements using the simple query protocol, returning the resulting rows.
    ///
    /// Statements should be separated by semicolons. If an error occurs, execution of the sequence will stop at that
    /// point. The simple query protocol returns the values in rows as strings rather than in their binary encodings,
    /// so the associated row type doesn't work with the `FromSql` trait. Rather than simply returning a list of the
    /// rows, this method returns a list of an enum which indicates either the completion of one of the commands,
    /// or a row of data. This preserves the framing between the separate statements in the request.
    ///
    /// # Warning
    ///
    /// Prepared statements should be use for any query which contains user-specified data, as they provided the
    /// functionality to safely embed that data in the request. Do not form statements via string concatenation and pass
    /// them to this method!
    #[inline]
    pub async fn simple_query(
        &self,
        query: &str,
    ) -> Result<Vec<SimpleQueryMessage>, Error> {
        Ok(self.0.simple_query(query).await?)
    }

    /// Executes a sequence of SQL statements using the simple query protocol.
    ///
    /// Statements should be separated by semicolons. If an error occurs, execution of the sequence will stop at that
    /// point. This is intended for use when, for example, initializing a database schema.
    ///
    /// # Warning
    ///
    /// Prepared statements should be use for any query which contains user-specified data, as they provided the
    /// functionality to safely embed that data in the request. Do not form statements via string concatenation and pass
    /// them to this method!
    #[inline]
    pub async fn batch_execute(&self, query: &str) -> Result<(), Error> {
        Ok(self.0.batch_execute(query).await?)
    }

    /// Begins a new database transaction.
    ///
    /// The transaction will roll back by default - use the `commit` method to commit it.
    #[inline]
    pub async fn transaction(&mut self) -> Result<Transaction<'_>, Error> {
        Ok(self.0.transaction().await?)
    }

    /// Returns a builder for a transaction with custom settings.
    ///
    /// Unlike the `transaction` method, the builder can be used to control the transaction's isolation level and other
    /// attributes.
    #[inline]
    pub fn build_transaction(&mut self) -> TransactionBuilder<'_> {
        self.0.build_transaction()
    }

    /// Constructs a cancellation token that can later be used to request
    /// cancellation of a query running on the connection associated with
    /// this client.
    #[inline]
    pub fn cancel_token(&self) -> CancelToken {
        self.0.cancel_token()
    }

    /// Determines if the connection to the server has already closed.
    ///
    /// In that case, all future queries will fail.
    #[inline]
    pub fn is_closed(&self) -> bool {
        self.0.is_closed()
    }
}
